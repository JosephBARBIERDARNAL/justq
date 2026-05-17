# justq

A tiny CLI that translates or corrects French/English text with a local Ollama model, with automatic copy to clipboard.

The installed command is `justq`.

<br>

## Requirements & installation

- Rust 1.85 or newer
- Ollama running locally
- A local model, for example:

```bash
ollama pull qwen2.5-coder:14b
```

Then install with:

```bash
cargo install --git https://github.com/y-sunflower/justq
```

<br>

## Usage

Correct text:

```bash
justq correct "i has a apple"
justq correct "je suis aller au bureau"
```

Translate between French and English (automatically detected):

```bash
justq translate "bonjour tout le monde"
justq translate "hello world"
```

Use a different default model:

```bash
export JUSTQ_MODEL="llama3:latest"
justq correct "i has a apple"
```

`--model` still overrides the environment for a single command:

```bash
justq --model qwen2.5-coder:14b translate "hello world"
```

<br>

> [!TIP]
> I personally added the following in my `~/.zshrc` file:

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
