# justq

A tiny CLI that:

- answers questions
- translates French/English text
- corrects French/English text

It only use local Ollama models, with **automatic copy to clipboard**.

This is a tool I built for my personal use for stuff that don't require advanced models. Let's avoid using Claude for translating basic text! I'm using an Apple M1 pro with 32 GB memory, and it works extremely well.

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

Ask a question:

```bash
justq ask "how do I handle errors in Rust?"
justq ask "when should I use a database transaction?"
```

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

Use a different default model:

```bash
export JUSTQ_MODEL="llama3:latest"
justq ask "how should I structure a small Rust CLI?"
```

`--model` still overrides the environment for a single command:

```bash
justq --model qwen2.5-coder:14b ask "explain Result versus Option"
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

# ask question
ja() {
  [[ -z "$*" ]] && return 1
  justq ask "$*"
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

- question

```
ja "what is the cleanest way to parse CLI arguments in Rust?"
```
