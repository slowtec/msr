# slowtec MSR - System Documentation

## Installation

Make sure

- [Rust](https://www.rust-lang.org/),
- [PlantUML](https://plantuml.com/) and
- [Graphviz](https://graphviz.org/)

are installed on your system.

Then you can install [mdBook](https://github.com/rust-lang/mdBook):

```sh
cargo install mdbook
cargo install mdbook-linkcheck
cargo install mdbook-plantuml
```

## Usage

The environment variable `RELATIVE_INCLUDE` has to be set to avoid fetching contents
from [C4-PlantUML on GitHub](https://github.com/plantuml-stdlib/C4-PlantUML)!

### Build

```sh
RELATIVE_INCLUDE=. mdbook build
```

### Serve interactively

```sh
RELATIVE_INCLUDE=. mdbook serve
```

For further information please look at the
[mdBook Documentation](https://rust-lang.github.io/mdBook/index.html).
