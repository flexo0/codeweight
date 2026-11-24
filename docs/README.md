# codeweight's docs

Using codeweight (the README is the short start):

- [usage.md](usage.md): the commands and options, the text report, which files are weighed, `show` and `diff`
- [json.md](json.md): the JSON report's fields, the machine contract
- [weights-file.md](weights-file.md): the weights file, the weight, each category in each language

The design, one file per subject. Each is a reference to what is built: what it is, why so, what is left. A decision made while
building goes into its subject's file, in the same commit.

- [cli.md](cli.md): the commands and options, the text and JSON reports
- [config.md](config.md): the weights file, where it is found, how it merges over the defaults, what is checked
- [weight.md](weight.md): a node's weight, nesting, each category's rules, how functions and classes are named
- [languages.md](languages.md): what TypeScript, TSX and JavaScript's, Python's, Rust's and Go's elements are, beyond the general rules
- [architecture.md](architecture.md): the modules and dependencies, the inputs, the language tables, the engine
- [commit.md](commit.md): how `show` and `diff` work out a commit's or a range's net delta
- [releasing.md](releasing.md): the version (what a break is, which bump), CHANGELOG.md, the steps of a release, the binaries a tag builds
