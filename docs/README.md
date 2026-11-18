# codeweight's design

One file per subject. Each is a reference to what is built: what it is, why so, what is left. A decision made while
building goes into its subject's file, in the same commit.

- [cli.md](cli.md): the commands and options, the text and JSON reports
- [config.md](config.md): the weights file, where it is found, how it merges over the defaults, what is checked
- [weight.md](weight.md): a node's weight, nesting, each category's rules, how functions and classes are named
- [languages.md](languages.md): what TypeScript, TSX and JavaScript's, Python's, Rust's and Go's elements are, beyond the general rules
- [architecture.md](architecture.md): the modules and dependencies, the inputs, the language tables, the engine
- [commit.md](commit.md): how `commit` works out a commit's or a range's net delta
- [milestones.md](milestones.md): how it was built, milestone by milestone, and the tuning of the defaults on real code
