# Vendored Tree-sitter grammars

Each directory under `vendor/` is compiled by `lux-syntax/build.rs`. `sql/src/parser.c` is not committed upstream; it was generated with tree-sitter-cli 0.26.3 (ABI 15) from the cloned grammar. Every grammar's `LANGUAGE_VERSION` is 13, 14, or 15, which tree-sitter 0.26 accepts.

Left out, and why:

- Perl, PowerShell, and LaTeX: the upstream repos do not commit `parser.c`, and generating them is a separate toolchain from the SQL grammar that already had a one-command generate.
- Vue, Svelte, and Astro need a host-language injection pipeline this crate does not run for embedded blocks.
- JSONC: no grammar repo with a committed parser was available; JSON stays the `.json` grammar.
- Solidity: the grammar repository was not found.
- OCaml `.mli` and the interface/type grammars: a second and third ~20MB parser for the same language. `.ml` uses the implementation grammar.
- Terraform: the same HCL parser. `.tf` and `.tfvars` map to HCL.
- PHP-only and DTD: subsets of the PHP and XML grammars already vendored.
- reStructuredText: the scanner is split across C files that call each other without declarations, and this clang rejects that as an implicit-function-declaration error.
- Vim: a 4.6MB parser whose highlight query is large and brittle relative to the languages above.
- Sass and Less: only SCSS is vendored.
- Zsh: no grammar; a `zsh` shebang still selects Bash, which is the previous behaviour.

Included grammars:

- `php` https://github.com/tree-sitter/tree-sitter-php `58d5643086ae60e93a3746cca648363ee97c8761`
- `ruby` https://github.com/tree-sitter/tree-sitter-ruby `ad907a69da0c8a4f7a943a7fe012712208da6dee`
- `sql` https://github.com/DerekStride/tree-sitter-sql `49a4357df5e89832c2d2808074e6c22a1ad7d857` (parser.c generated with tree-sitter-cli 0.26.3)
- `xml` https://github.com/tree-sitter-grammars/tree-sitter-xml `5000ae8f22d11fbe93939b05c1e37cf21117162d`
- `dockerfile` https://github.com/camdencheek/tree-sitter-dockerfile `971acdd908568b4531b0ba28a445bf0bb720aba5`
- `make` https://github.com/alemuller/tree-sitter-make `a4b9187417d6be349ee5fd4b6e77b4172c6827dd`
- `lua` https://github.com/tree-sitter-grammars/tree-sitter-lua `10fe0054734eec83049514ea2e718b2a56acd0c9`
- `c_sharp` https://github.com/tree-sitter/tree-sitter-c-sharp `8c0abe0b84a3681d3e6852ce2e2a1eecb4f731c5`
- `diff` https://github.com/the-mikedavis/tree-sitter-diff `ada384ac7bfc1307f32de474620120add29998fb`
- `markdown_inline` https://github.com/tree-sitter-grammars/tree-sitter-markdown `41ff24a6c775492c543b09228aeb98d1cd2b3a0f`
- `zig` https://github.com/tree-sitter-grammars/tree-sitter-zig `6479aa13f32f701c383083d8b28360ebd682fb7d`
- `graphql` https://github.com/bkegley/tree-sitter-graphql `5e66e961eee421786bdda8495ed1db045e06b5fe`
- `ini` https://github.com/justinmk/tree-sitter-ini `e4018b5176132b4f3c5d6e61cea383f42288d0f5`
- `proto` https://github.com/mitchellh/tree-sitter-proto `42d82fa18f8afe59b5fc0b16c207ee4f84cb185f`
- `hcl` https://github.com/tree-sitter-grammars/tree-sitter-hcl `64ad62785d442eb4d45df3a1764962dafd5bc98b`
- `nix` https://github.com/nix-community/tree-sitter-nix `e49c2f317e896eb08967717e758a46d6eea038b2`
- `cmake` https://github.com/uyha/tree-sitter-cmake `a3ec2b2144daf7132e7ee824d8cab9610523d10f`
- `gitignore` https://github.com/shunsambongi/tree-sitter-gitignore `f4685bf11ac466dd278449bcfe5fd014e94aa504`
- `scss` https://github.com/tree-sitter-grammars/tree-sitter-scss `5da5ba71a558b007b352c505bfcd9095ae337022`
- `prisma` https://github.com/victorhqc/tree-sitter-prisma `3556b2c1f20ec9ac91e92d32c43d9d2a0ca3cc49`
- `fish` https://github.com/ram02z/tree-sitter-fish `b7f1d682941e0c62dfcd6bf9ef481638351bab16`
- `scheme` https://github.com/6cdh/tree-sitter-scheme `45c72463ac4260ef6e07d1c0642ed43d4e9957b4`
- `clojure` https://github.com/sogaiu/tree-sitter-clojure `e43eff80d17cf34852dcd92ca5e6986d23a7040f`
- `regex` https://github.com/tree-sitter/tree-sitter-regex `b2ac15e27fce703d2f37a79ccd94a5c0cbe9720b`
- `erlang` https://github.com/WhatsApp/tree-sitter-erlang `62c82e67b73e13f3ef06e4c8c07ff035a8235d30`
- `gleam` https://github.com/gleam-lang/tree-sitter-gleam `fa6d0d94804f1b342e175865a3dbeb316ee5115e`
- `r` https://github.com/r-lib/tree-sitter-r `58a22794466c0fc15b0d3b40531db751593721e8`
- `elixir` https://github.com/elixir-lang/tree-sitter-elixir `cb32ccbe17c67037b09e31bf9beb22cdae6ed8e9`
- `scala` https://github.com/tree-sitter/tree-sitter-scala `d86ab76fd7e8e04cd1da5cb14d7d5adc143eec78`
- `dart` https://github.com/UserNobody14/tree-sitter-dart `0038280b774b65c1b89ba7fec50624d8baed9fe1`
- `julia` https://github.com/tree-sitter/tree-sitter-julia `e0f9dcd180fdcfcfa8d79a3531e11d99e79321d3`
- `haskell` https://github.com/tree-sitter/tree-sitter-haskell `97288e585b0bd44199720280d067fa35bacd81ff`
- `ocaml` https://github.com/tree-sitter/tree-sitter-ocaml `3b2e14e0697d405c9aa0beddfa09b71f45abc504`
- `commonlisp` https://github.com/tree-sitter-grammars/tree-sitter-commonlisp `32323509b3d9fe96607d151c2da2c9009eb13a2f`
