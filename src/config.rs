//! The weights file: found, merged over the built-in defaults and validated.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::lang::{self, Lang};
use crate::source;

/// The built-in defaults, written out by `codeweight init`.
pub const DEFAULTS: &str = include_str!("default.toml");

/// The weights file's name, looked up in the current folder and at the repo's root.
pub const FILE_NAME: &str = "codeweight.toml";

/// What an element of code is, for its base weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Statement,
    Call,
    /// A call's argument, each one (a call's arguments field: a list's items, or a lone one).
    Argument,
    Branch,
    /// A `switch` / `match` case, as `branch` is an `if`.
    Case,
    Loop,
    Try,
    Catch,
    Lambda,
    Function,
    Class,
    BoolOp,
    Jump,
    Operator,
    Access,
    /// A function's or lambda's parameter, weighed by its place (`[parameters]`).
    Parameter,
}

impl Category {
    pub const ALL: [Category; 16] = [
        Category::Statement,
        Category::Call,
        Category::Argument,
        Category::Branch,
        Category::Case,
        Category::Loop,
        Category::Try,
        Category::Catch,
        Category::Lambda,
        Category::Function,
        Category::Class,
        Category::BoolOp,
        Category::Jump,
        Category::Operator,
        Category::Access,
        Category::Parameter,
    ];

    /// Its key in the weights file.
    pub fn name(self) -> &'static str {
        match self {
            Category::Statement => "statement",
            Category::Call => "call",
            Category::Argument => "argument",
            Category::Branch => "branch",
            Category::Case => "case",
            Category::Loop => "loop",
            Category::Try => "try",
            Category::Catch => "catch",
            Category::Lambda => "lambda",
            Category::Function => "function",
            Category::Class => "class",
            Category::BoolOp => "bool_op",
            Category::Jump => "jump",
            Category::Operator => "operator",
            Category::Access => "access",
            Category::Parameter => "parameter",
        }
    }

    pub fn from_name(name: &str) -> Option<Category> {
        Category::ALL.into_iter().find(|c| c.name() == name)
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// How a node's weight grows with its depth.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Model {
    /// `1 + factor * depth`
    Linear,
    /// `factor ^ depth`
    Exponential,
}

/// The `[nesting]` table.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Nesting {
    pub model: Model,
    pub factor: f64,
    pub max_depth: u32,
    /// The categories that weigh the same at any depth: the parts of a statement, whose statement already weighs
    /// its depth.
    pub flat: Vec<Category>,
}

impl Nesting {
    /// The multiplier of a `category`'s base weight at `depth`: 1 for a flat one.
    pub fn factor_of(&self, category: Category, depth: u32) -> f64 {
        if self.flat.contains(&category) {
            1.0
        } else {
            self.factor_at(depth)
        }
    }

    /// The multiplier of a node's base weight at `depth`, capped at `max_depth`.
    pub fn factor_at(&self, depth: u32) -> f64 {
        let depth = depth.min(self.max_depth);
        match self.model {
            Model::Linear => 1.0 + self.factor * f64::from(depth),
            Model::Exponential => self.factor.powi(depth as i32),
        }
    }
}

/// The `[parameters]` table: how a parameter's weight grows with its place, so a long list weighs more than its
/// count.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Parameters {
    /// What each place adds to the multiplier.
    pub step: f64,
    /// The multiplier's cap.
    pub max: f64,
}

impl Parameters {
    /// The multiplier of the `parameter` weight for the parameter at `place`, from 0: `1 + step * place`, capped at
    /// `max`.
    pub fn factor_at(&self, place: usize) -> f64 {
        (1.0 + self.step * place as f64).min(self.max)
    }
}

/// The `[conditions]` table: rules that weigh what a node's kind alone doesn't tell, each weighing as an existing
/// category. All off when a file on its own leaves the table out, so it weighs as the categories alone do; the
/// defaults turn four on (the tuning on the example pairs).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Conditions {
    /// A cast or type assertion (`x as T`, `(T) x`, `x!`, `<T>x`, `x.(T)`) is an `operator`.
    pub casts: bool,
    /// A pattern tested in a condition (Rust's `if let`, `while let`) is an `operator`, as a comparison is.
    pub let_patterns: bool,
    /// A number literal but 0 and 1 is an `access`, as the named constant in its place would be.
    pub magic_numbers: bool,
    /// Each unary operator counts, no run of them: `!!x` is 2.
    pub each_unary: bool,
    /// `??` and `??=` are an `operator`, not a `bool_op`: a default value, no branch.
    pub nullish_operator: bool,
}

/// The `[files]` table: which files a folder walk weighs, and which are tests.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Files {
    pub exclude: Vec<String>,
    pub test_globs: Vec<String>,
}

/// A language's weights: `[weights]` with its `[languages.<name>.weights]` over them, and its node kinds remapped.
#[derive(Clone, Debug, PartialEq)]
pub struct LangConfig {
    weights: [f64; Category::ALL.len()],
    /// A tree-sitter node kind mapped to a category, or to none, over the language's own table.
    pub node_kinds: BTreeMap<String, Option<Category>>,
}

impl LangConfig {
    pub fn weight(&self, category: Category) -> f64 {
        self.weights[category.index()]
    }
}

/// The weights file, merged and validated.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub nesting: Nesting,
    pub parameters: Parameters,
    pub conditions: Conditions,
    pub files: Files,
    languages: BTreeMap<&'static str, LangConfig>,
}

impl Config {
    /// The built-in defaults.
    pub fn defaults() -> Result<Config> {
        Config::from_value(parse_toml(DEFAULTS).context("in the built-in defaults")?)
            .context("in the built-in defaults")
    }

    /// The weights file at `path` merged over the defaults, or the defaults when there is none.
    pub fn load(path: Option<&Path>) -> Result<Config> {
        let Some(path) = path else {
            return Config::defaults();
        };
        let text = fs::read_to_string(path)
            .with_context(|| format!("reading the weights file {}", path.display()))?;
        Config::from_str_over_defaults(&text)
            .with_context(|| format!("in the weights file {}", path.display()))
    }

    /// A weights file's text merged over the defaults.
    pub fn from_str_over_defaults(text: &str) -> Result<Config> {
        let mut merged = parse_toml(DEFAULTS).context("in the built-in defaults")?;
        merge(&mut merged, parse_toml(text)?);
        Config::from_value(merged)
    }

    /// A complete weights file on its own, no defaults under it (the engine's tests use it).
    #[cfg(test)]
    pub fn from_str_alone(text: &str) -> Result<Config> {
        Config::from_value(parse_toml(text)?)
    }

    /// The weights of `lang`.
    pub fn lang(&self, lang: &Lang) -> &LangConfig {
        // Every language of `lang::ALL` gets an entry in `from_value`.
        &self.languages[lang.name]
    }

    fn from_value(table: toml::Table) -> Result<Config> {
        let raw: RawConfig = toml::Value::Table(table).try_into()?;
        check_non_negative("[nesting] factor", raw.nesting.factor)?;
        check_non_negative("[parameters] step", raw.parameters.step)?;
        check_non_negative("[parameters] max", raw.parameters.max)?;
        for glob in &raw.files.exclude {
            source::glob(glob).with_context(|| format!("[files] exclude `{glob}`"))?;
        }
        for glob in &raw.files.test_globs {
            source::glob(glob).with_context(|| format!("[files] test_globs `{glob}`"))?;
        }

        let mut base = [0.0; Category::ALL.len()];
        let mut given = [false; Category::ALL.len()];
        for (key, value) in &raw.weights {
            let category = category("[weights]", key)?;
            check_non_negative(&format!("[weights] {key}"), *value)?;
            base[category.index()] = *value;
            given[category.index()] = true;
        }
        if let Some(missing) = Category::ALL.iter().find(|c| !given[c.index()]) {
            bail!("[weights] has no weight for `{}`", missing.name());
        }

        for name in raw.languages.keys() {
            if lang::by_key(name).next().is_none() {
                bail!(
                    "unknown language `{name}` in [languages], expected one of: {}",
                    lang::keys().join(", ")
                );
            }
        }
        let mut languages = BTreeMap::new();
        for lang in lang::ALL {
            let raw_lang = raw.languages.get(lang.key);
            let mut weights = base;
            let mut node_kinds = BTreeMap::new();
            if let Some(raw_lang) = raw_lang {
                let table = format!("[languages.{}.weights]", lang.key);
                for (key, value) in &raw_lang.weights {
                    let category = category(&table, key)?;
                    check_non_negative(&format!("{table} {key}"), *value)?;
                    weights[category.index()] = *value;
                }
                // A kind of any grammar of the key (TSX's JSX ones are TSX's only); a grammar without it ignores it.
                let table = format!("[languages.{}.node_kinds]", lang.key);
                let grammars: Vec<_> = lang::by_key(lang.key).map(|l| (l.grammar)()).collect();
                for (kind, target) in &raw_lang.node_kinds {
                    if !grammars.iter().any(|g| lang::is_named_kind(g, kind)) {
                        bail!("unknown node kind `{kind}` in {table}");
                    }
                    let target = match target.as_str() {
                        "none" => None,
                        name => Some(category(&format!("{table} {kind}"), name)?),
                    };
                    node_kinds.insert(kind.clone(), target);
                }
            }
            languages.insert(
                lang.name,
                LangConfig {
                    weights,
                    node_kinds,
                },
            );
        }

        Ok(Config {
            nesting: raw.nesting,
            parameters: raw.parameters,
            conditions: raw.conditions,
            files: raw.files,
            languages,
        })
    }
}

/// The weights file to use, the first found: `explicit` (`--config`), `codeweight.toml` in `cwd`, `codeweight.toml`
/// at the root of the repo `inside` is in; `None` for the defaults.
pub fn find(explicit: Option<&Path>, cwd: &Path, inside: Option<&Path>) -> Result<Option<PathBuf>> {
    if let Some(path) = explicit {
        if !path.is_file() {
            bail!("the weights file {} doesn't exist", path.display());
        }
        return Ok(Some(path.to_path_buf()));
    }
    let local = cwd.join(FILE_NAME);
    if local.is_file() {
        return Ok(Some(local));
    }
    if let Some(root) = inside.and_then(|path| repo_root(&cwd.join(path))) {
        let at_root = root.join(FILE_NAME);
        if at_root.is_file() {
            return Ok(Some(at_root));
        }
    }
    Ok(None)
}

/// The root of the git repo `path` is in: the nearest folder up from it that holds `.git`.
pub fn repo_root(path: &Path) -> Option<PathBuf> {
    let path = path.canonicalize().ok()?;
    path.ancestors()
        .find(|dir| dir.join(".git").exists())
        .map(Path::to_path_buf)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    nesting: Nesting,
    parameters: Parameters,
    #[serde(default)]
    conditions: Conditions,
    weights: BTreeMap<String, f64>,
    #[serde(default)]
    languages: BTreeMap<String, RawLang>,
    files: Files,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLang {
    #[serde(default)]
    weights: BTreeMap<String, f64>,
    #[serde(default)]
    node_kinds: BTreeMap<String, String>,
}

fn parse_toml(text: &str) -> Result<toml::Table> {
    Ok(text.parse::<toml::Table>()?)
}

/// Merges `over` into `base`: tables key by key, any other value replaced whole (an array too).
fn merge(base: &mut toml::Table, over: toml::Table) {
    for (key, value) in over {
        match (base.get_mut(&key), value) {
            (Some(toml::Value::Table(base_table)), toml::Value::Table(over_table)) => {
                merge(base_table, over_table)
            }
            (_, value) => {
                base.insert(key, value);
            }
        }
    }
}

fn category(table: &str, name: &str) -> Result<Category> {
    Category::from_name(name).with_context(|| {
        format!(
            "unknown category `{name}` in {table}, expected one of: {}",
            Category::ALL.map(Category::name).join(", ")
        )
    })
}

fn check_non_negative(what: &str, value: f64) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        bail!("{what} is {value}, expected a number 0 or above");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::java::JAVA;

    #[test]
    fn defaults_load_and_set_every_category() {
        let config = Config::defaults().unwrap();
        assert_eq!(config.nesting.model, Model::Linear);
        assert_eq!(config.lang(&JAVA).weight(Category::Statement), 1.0);
    }

    #[test]
    fn a_user_file_changes_only_what_it_holds() {
        let defaults = Config::defaults().unwrap();
        let config = Config::from_str_over_defaults(
            "[weights]\ncall = 7\nlambda = 10\n[files]\nexclude = [\"a/**\"]\n",
        )
        .unwrap();
        assert_eq!(config.lang(&JAVA).weight(Category::Call), 7.0);
        // No default per-language weight shadows a base weight the file sets.
        assert_eq!(config.lang(&JAVA).weight(Category::Lambda), 10.0);
        assert_eq!(
            config.lang(&JAVA).weight(Category::Loop),
            defaults.lang(&JAVA).weight(Category::Loop)
        );
        assert_eq!(config.files.exclude, vec!["a/**".to_string()]);
        assert_eq!(config.files.test_globs, defaults.files.test_globs);
        assert_eq!(config.nesting, defaults.nesting);
    }

    #[test]
    fn a_language_overrides_the_base_weights() {
        let config = Config::from_str_over_defaults(
            "[weights]\nbranch = 4\n[languages.java.weights]\nbranch = 9\n",
        )
        .unwrap();
        assert_eq!(config.lang(&JAVA).weight(Category::Branch), 9.0);
    }

    #[test]
    fn node_kinds_map_to_a_category_or_none() {
        let config = Config::from_str_over_defaults(
            "[languages.java.node_kinds]\nlabeled_statement = \"jump\"\nexpression_statement = \"none\"\n",
        )
        .unwrap();
        let kinds = &config.lang(&JAVA).node_kinds;
        assert_eq!(kinds["labeled_statement"], Some(Category::Jump));
        assert_eq!(kinds["expression_statement"], None);
    }

    fn error_of(text: &str) -> String {
        format!("{:#}", Config::from_str_over_defaults(text).unwrap_err())
    }

    #[test]
    fn an_unknown_key_is_an_error() {
        assert!(error_of("[nesting]\nfactr = 1\n").contains("factr"));
        assert!(error_of("[files]\nexclud = []\n").contains("exclud"));
        assert!(error_of("[colors]\na = 1\n").contains("colors"));
        assert!(error_of("[languages.java]\nweight = {}\n").contains("weight"));
    }

    #[test]
    fn an_unknown_category_is_an_error() {
        assert!(error_of("[weights]\nloops = 1\n").contains("unknown category `loops`"));
        assert!(
            error_of("[languages.java.weights]\nfunc = 1\n").contains("unknown category `func`")
        );
        assert!(
            error_of("[languages.java.node_kinds]\nif_statement = \"iff\"\n")
                .contains("unknown category `iff`")
        );
    }

    #[test]
    fn an_unknown_node_kind_is_an_error() {
        assert!(
            error_of("[languages.java.node_kinds]\nif_stmt = \"branch\"\n")
                .contains("unknown node kind `if_stmt`")
        );
    }

    #[test]
    fn an_unknown_language_is_an_error() {
        let error = error_of("[languages.cobol.weights]\nloop = 1\n");
        assert!(
            error.contains(
                "unknown language `cobol` in [languages], expected one of: java, typescript, javascript, python, rust, go"
            ),
            "{error}"
        );
        assert!(error_of("[languages.tsx.weights]\nloop = 1\n").contains("unknown language `tsx`"));
    }

    #[test]
    fn typescript_s_table_is_tsx_s_too() {
        use crate::lang::typescript::{TSX, TYPESCRIPT};
        let config = Config::from_str_over_defaults(
            "[languages.typescript.weights]\nloop = 9\n\
             [languages.typescript.node_kinds]\njsx_element = \"none\"\ntype_assertion = \"call\"\n",
        )
        .unwrap();
        assert_eq!(config.lang(&TYPESCRIPT).weight(Category::Loop), 9.0);
        assert_eq!(config.lang(&TSX).weight(Category::Loop), 9.0);
        // A kind of one grammar of the two is valid; the other grammar has no node of it.
        assert_eq!(config.lang(&TSX).node_kinds["jsx_element"], None);
        assert_eq!(
            config.lang(&TSX).node_kinds["type_assertion"],
            Some(Category::Call)
        );
        assert!(
            error_of("[languages.typescript.node_kinds]\njsx_elem = \"call\"\n")
                .contains("unknown node kind `jsx_elem` in [languages.typescript.node_kinds]")
        );
    }

    #[test]
    fn javascript_s_table_is_its_own() {
        use crate::lang::javascript::JAVASCRIPT;
        use crate::lang::typescript::TYPESCRIPT;
        let config = Config::from_str_over_defaults(
            "[languages.typescript.weights]\nloop = 9\n\
             [languages.javascript.weights]\nloop = 7\n\
             [languages.javascript.node_kinds]\nfield_definition = \"none\"\n",
        )
        .unwrap();
        assert_eq!(config.lang(&TYPESCRIPT).weight(Category::Loop), 9.0);
        assert_eq!(config.lang(&JAVASCRIPT).weight(Category::Loop), 7.0);
        assert_eq!(
            config.lang(&JAVASCRIPT).node_kinds["field_definition"],
            None
        );
        assert!(config.lang(&TYPESCRIPT).node_kinds.is_empty());
        // A TypeScript kind is no JavaScript one.
        assert!(
            error_of("[languages.javascript.node_kinds]\ninterface_declaration = \"call\"\n")
                .contains(
                    "unknown node kind `interface_declaration` in [languages.javascript.node_kinds]"
                )
        );
    }

    #[test]
    fn a_bad_glob_is_an_error() {
        let error = error_of("[files]\nexclude = [\"a/[b\"]\n");
        assert!(error.contains("[files] exclude `a/[b`"), "{error}");
    }

    #[test]
    fn a_bad_value_is_an_error() {
        assert!(error_of("[weights]\nloop = -1\n").contains("0 or above"));
        assert!(error_of("[weights]\nloop = \"3\"\n").contains("invalid type"));
        assert!(error_of("[nesting]\nmodel = \"cubic\"\n").contains("cubic"));
        assert!(error_of("[nesting]\nflat = [\"calls\"]\n").contains("calls"));
        assert!(error_of("[parameters]\nstep = -0.1\n").contains("[parameters] step"));
    }

    #[test]
    fn the_defaults_hold_every_condition_four_on() {
        let defaults = parse_toml(DEFAULTS).unwrap();
        let table = defaults["conditions"].as_table().unwrap();
        assert_eq!(table.len(), 5);
        assert_eq!(
            Config::defaults().unwrap().conditions,
            Conditions {
                casts: true,
                let_patterns: true,
                magic_numbers: true,
                each_unary: true,
                nullish_operator: false,
            }
        );
        let mine = Config::from_str_over_defaults("[conditions]\ncasts = false\n").unwrap();
        assert!(!mine.conditions.casts && mine.conditions.each_unary);
        assert!(error_of("[conditions]\ncast = true\n").contains("cast"));
    }

    #[test]
    fn a_file_on_its_own_must_set_every_category() {
        let error = Config::from_str_alone(
            "[nesting]\nmodel = \"linear\"\nfactor = 1\nmax_depth = 3\nflat = []\n[parameters]\nstep = 0\nmax = 1\n[weights]\nstatement = 1\n\
             [files]\nexclude = []\ntest_globs = []\n",
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("no weight for `call`"));
    }

    #[test]
    fn nesting_models() {
        let linear = Nesting {
            model: Model::Linear,
            factor: 0.5,
            max_depth: 2,
            flat: vec![Category::Call],
        };
        assert_eq!(linear.factor_at(0), 1.0);
        assert_eq!(linear.factor_at(1), 1.5);
        assert_eq!(linear.factor_at(5), 2.0);
        assert_eq!(linear.factor_of(Category::Statement, 1), 1.5);
        assert_eq!(linear.factor_of(Category::Call, 1), 1.0);
        let exponential = Nesting {
            model: Model::Exponential,
            factor: 2.0,
            max_depth: 3,
            flat: vec![],
        };
        assert_eq!(exponential.factor_at(0), 1.0);
        assert_eq!(exponential.factor_at(2), 4.0);
        assert_eq!(exponential.factor_at(9), 8.0);
    }

    #[test]
    fn a_parameter_s_factor_grows_with_its_place_to_its_cap() {
        let parameters = Parameters {
            step: 0.2,
            max: 2.0,
        };
        let factors: Vec<f64> = (0..8).map(|place| parameters.factor_at(place)).collect();
        let expected = [1.0, 1.2, 1.4, 1.6, 1.8, 2.0, 2.0, 2.0];
        for (factor, expected) in factors.iter().zip(expected) {
            assert!((factor - expected).abs() < 1e-9, "{factors:?}");
        }
    }

    #[test]
    fn the_lookup_order() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let cwd = root.join("cwd");
        let repo = root.join("repo");
        fs::create_dir_all(&cwd).unwrap();
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(repo.join("src")).unwrap();
        let explicit = root.join("mine.toml");
        fs::write(&explicit, "").unwrap();

        assert_eq!(find(None, &cwd, Some(&repo.join("src"))).unwrap(), None);
        fs::write(repo.join(FILE_NAME), "").unwrap();
        assert_eq!(
            find(None, &cwd, Some(&repo.join("src"))).unwrap(),
            Some(repo.canonicalize().unwrap().join(FILE_NAME))
        );
        fs::write(cwd.join(FILE_NAME), "").unwrap();
        assert_eq!(
            find(None, &cwd, Some(&repo.join("src"))).unwrap(),
            Some(cwd.join(FILE_NAME))
        );
        assert_eq!(
            find(Some(&explicit), &cwd, Some(&repo.join("src"))).unwrap(),
            Some(explicit.clone())
        );
        assert!(find(Some(&root.join("missing.toml")), &cwd, None).is_err());
    }
}
