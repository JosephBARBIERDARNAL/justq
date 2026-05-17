# justq

A tiny Rust CLI that translates or corrects French/English text with a local
Ollama model.

The installed command is `justq`.

## Requirements

- Rust 1.85 or newer
- Ollama running locally
- A local model, for example:

```bash
ollama pull qwen2.5-coder:14b
```

## Install

```bash
cargo install --git https://github.com/y-sunflower/justq
```

## Usage

Correct text:

```bash
justq correct "i has a apple"
justq correct "je suis aller au bureau"
```

Translate between French and English:

```bash
justq translate "bonjour tout le monde"
justq translate "hello world"
```

I personally added the following in my `~/.zshrc` file:

```bash
# translation
jt() {
  [[ -z "$*" ]] && return 1
  justq translate "$*"
}

# correction
jc() {
  [[ -z "$*" ]] && return 1
  justq correct "$*"
}
```

This lets me do:

- correction

```
jc "voici mon nouveau project python: un cli tool pour lanalyse de donnees"
```

- translation

```
jt "A Rust CLI that translates French/English text with a local model."
```
