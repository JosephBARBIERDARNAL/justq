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

Force a language when the text is ambiguous:

```bash
justq correct --language english "i has a apple"
justq correct --language french "je suis aller au bureau"
justq translate --to english "bonjour tout le monde"
justq translate --to french "hello world"
```

Short aliases:

```bash
justq fix "i has a apple"
justq t --to fr "hello world"
```

Pipe text from stdin:

```bash
echo "i has a apple" | justq correct
echo "bonjour tout le monde" | justq translate --to en
```

The model output is copied to your clipboard automatically. The clipboard only
receives the raw corrected or translated text, not the pretty terminal title.

Disable clipboard copy with:

```bash
justq --no-copy correct "i has a apple"
justq translate --to french --no-copy "hello world"
```

By default, `justq` pretty-prints the response in an interactive terminal and
shows a small waiting spinner on stderr while the local model is thinking. It
prints plain Markdown when stdout is piped. Force plain Markdown with:

```bash
justq --raw correct "i has a apple"
```

Use another model:

```bash
justq --model llama3:latest correct "i has a apple"
```

Use another Ollama server:

```bash
justq --ollama-url http://localhost:11434 translate "hello world"
```

Configuration can also come from environment variables:

```bash
OLLAMA_MODEL=llama3:latest justq correct "i has a apple"
OLLAMA_HOST=http://localhost:11434 justq translate "hello world"
OLLAMA_URL=http://localhost:11434 justq translate "hello world"
```

## Short Alias

The project does not install a binary named `jq` by default because that would
conflict with the popular JSON processor. If you do not use that command, add a
local shell alias:

```bash
alias jq='justq'
```

Then you can run:

```bash
jq correct "i has a apple"
```

## Build From Source

```bash
git clone https://github.com/y-sunflower/justq.git
cd justq
cargo build --release
```
