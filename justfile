default: make

alias m := make
alias s := serve

serve:
	python3 -m http.server 8000

make:
	wasm-pack build --target web
