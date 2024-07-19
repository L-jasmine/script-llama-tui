# Script Llama Tui

The main objective of this project is to explore how ordinary llama models can acquire the ability to make tool calls.

## Quick Start


###  Clone the repository
```
git clone https://github.com/L-jasmine/script-llama-tui
```

### Download Llama model.
```
wget https://huggingface.co/second-state/Llama-3-8B-Instruct-GGUF/resolve/main/Meta-Llama-3-8B-Instruct-Q5_K_M.gguf
```

### Configure Environment Variables
This project uses dynamic linking to connect to llama.cpp, so it is necessary to download or compile the llama.cpp dynamic link library in advance.

Before running the project, you need to configure environment variables to specify the location of the Llama library and the search path for dynamic link libraries. Please follow the steps below:

```shell
export LLAMA_LIB={LLama_Dynamic_Library_Dir}
# export LD_LIBRARY_PATH={LLama_Dynamic_Library_Dir}
```

### Run

Use the following command to run the example program:

```shell
cargo run -- --model-path Meta-Llama-3-8B-Instruct-Q5_K_M.gguf --model-type llama3 --prompt-path static/prompt.lua.toml -e lua -c 2048 -n 128
```

## Contributions

We welcome any form of contributions, including bug reports, new feature suggestions, and code submissions.

## License

This project is licensed under the MIT License.