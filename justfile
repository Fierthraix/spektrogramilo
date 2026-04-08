default: make

alias m := make
alias s := serve
alias ms := make-static
alias ih := install-hooks

serve:
	python3 -m http.server 8000

make:
	wasm-pack build --target web

make-static:
	cargo run --quiet --manifest-path xtask/Cargo.toml --

check:
	./.githooks/pre-commit

install-hooks:
	git config core.hooksPath .githooks

web:
	@just make
	rsync -avx --exclude='target' --exclude='.git' . vps:projects/spektrogramilo
