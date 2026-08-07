# ffi_c

`ffi_c` は `microps` の domain stack を C から利用するための static library です。Rust 側が単一の `Stack<CPlatform>` を所有し、C 側は platform 操作と Ethernet driver の callback だけを提供します。

## 初期化

C 側は `MicropsPlatform` を用意して `microps_init()` に渡します。構造体内の callback はすべて必須です（ログが不要な場合は no-op callback を設定します）。`context` はすべての callback にそのまま渡されます。

初期化後、`microps_ethernet_register()`、`microps_ipv4_register()`、`microps_ipv4_default_gateway()` を呼び、最後に `microps_start()` を呼びます。device と interface の登録は start 前に行います。

`microps_shutdown()` は待機中の socket を中断して device を閉じます。xv6 の通常の kernel lifetime では呼び出しません。

## Ethernet 入出力

Ethernet driver は割り込みで frame を直接 Rust に渡しません。割り込み handler は xv6 側の receive queue に保存し、process context で次を呼びます。

```text
microps_device_receive()
microps_poll()
```

`microps_device_receive()` は payload を Rust 側へコピーします。`transmit` callback に渡される buffer は callback の実行中だけ有効であり、非同期に保持する場合は C 側でコピーします。

`microps_device_receive()`、`microps_poll()`、`microps_tick()`、socket API は割り込み context から呼び出してはいけません。

## 待機と interruption

`mutex_wait` は mutex を保持した状態で呼ばれ、sleep 中に mutex を解放し、起床後に再取得してから戻ります。戻り値 `1` は interruption を表します。xv6 側では、現在の process が killed された場合にも `1` を返すようにします。

`mutex_interrupt_all` は通常の wakeup と異なり、待機中の callback が interruption を返す状態にしてから全 waiter を起こします。

socket API の負の戻り値は `MICROPS_*` の error code です。正の戻り値は送受信した byte 数です。socket open は out parameter に handle を書き、戻り値で成否を返します。Rust の型や構造体の内部表現は C ABI に公開されません。device、interface、socket は `uint64_t` の opaque handle として扱います。

## build

通常の build では static library が生成されます。xv6 向けに static library と C header を同時に出力する場合は xtask を使います。

```sh
cargo xtask build ffi_c --target <target>
cargo xtask build ffi_c --target <target> --release
```

出力先はそれぞれ `dist/ffi_c/<target>/debug/` と `dist/ffi_c/<target>/release/` です。C header は公開された `#[repr(C)]` 型と `extern "C"` 関数から cbindgen で生成します。
