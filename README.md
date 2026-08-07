個人メモ

- crates/
  - domain
    - ライブラリクレートのみ
    - no_std を利用、trait でプラットフォーム依存の部分を分離
    - poll() までを提供
    - テストではモックした platform を利用
    - import する時に domain という名前になるのが嫌なので package 名を Cargo.toml で `microps` などにする
  - platform/
    - linux
      - バイナリクレートとライブラリクレート
      - バイナリクレートでは poll() を呼び出すループを含む。Ctrl+C で止める
      - (予定) テストでは linux の trait 実装を利用した e2e テストを行う
        - run と test の初期化処理や後処理を共通化する
        - とすると、custom test frameworks を利用した方が良いかも
          - 初期化処理 → テスト `fn()` を順に実行 → 後処理
    - ffi_c (予定)
      - ライブラリクレートのみ、staticlib
      - no_std を利用、C とリンクするための FFI 層
      - domain 層で定義した trait と FFI の間を作る
      - 例えば C から関数ポインタをもらって呼び出すなど
      - ビルドによって `.a`, `.h` ファイルを出力
- 使われ方
  - linux:
    - run: `cargo run -p linux`
    - test: `cargo test -p linux`
    - build: `cargo build -p linux`
  - xv6 (予定):
    - build: `cargo xtask build ffi_c --target <target>` (`cargo xtask build ffi_c --target <target> --release`)
      - xv6-riscv 用の例: `cargo xtask build ffi_c --target riscv64imac-unknown-none-elf --release`
      - 事前に対象 target を追加する: `rustup target add riscv64imac-unknown-none-elf`
    - 標準の `cargo build` では `.a` までしか生成されないため xtask を用いる
      - xtask 内で `.h` は `cbindgen` を用いて生成し、`dist/ffi_c/<target>/<profile>/` に `.a` とともに出力
  - Cyrius (自作 OS, 予定):
    - lib クレートとしてこのリポジトリの domain (microps) を利用。platform/ は利用しない
    - 例えば Cyrius の Cargo.toml から packages に microps = { git=... } のように書けると良い。microps という package 名は `domain` を指すのでこちらが import される

## Linux TAP の実行

TAP device の作成と Linux host 側の address/link 設定は、Rust の実行とは分離して行う。
初回または TAP を削除した後は、次のように準備する。

```sh
./scripts/linux_tap_up.sh
cargo run -p linux
```

`linux_tap_up.sh` は TAP device を実行ユーザー所有で作成するため、sudo はこの準備時だけ使用する。
Rust 側はユーザー権限で既存の TAP device に接続する。

終了後に TAP device も削除する場合は、次を実行する。

```sh
./scripts/linux_tap_down.sh
```

`linux_tap_up.sh` は Linux host に `10.0.0.1/24` を設定する。microps 側の `10.0.0.2/24` は protocol stack が設定する。これらの値と TAP name は `main.rs` と `linux_tap_up.sh` の間で対応している。
