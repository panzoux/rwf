# /project:check — Rust 品質チェック一式

引数: 任意のテスト名フィルタ（例: `/project:check help_viewer`）。

rwf の品質チェックを正しい順序・正しいパラメータで実行する。フルスイート37分・OOM・
rwf-bin テストの stale 参照という既知の落とし穴を避けるための定型手順。

## 手順

1. **コンパイル確認（stale 参照検出）**
   `cargo test -p rwf --no-run`
   メソッド削除/リネームを伴うリファクタ後はここが最初に壊れる。

2. **lint（警告ゼロ維持）**
   `cargo clippy --all-targets -- -D warnings`
   警告が出たら原則その場で修正する（許容する場合は `#[allow]` + 理由コメント）。

3. **スモークテスト**
   `cargo test -p rwf -- --test-threads=1`（rwf-bin 51件、数秒）
   引数フィルタがあれば加えて `cargo test -p rwf-lib <filter> -- --test-threads=1`

4. **フル rwf-lib スイート（必要時のみ、~37分）**
   変更が広範囲に及ぶ場合のみ、バックグラウンドで:
   `cargo test -p rwf-lib -- --test-threads=1`
   実行中は他の cargo コマンドを打たない（ビルドロック競合）。

5. **結果判定**
   ベースラインは **失敗ゼロ**（2026-07-03 再測定、`.claude/CLAUDE.local.md` 参照）。
   失敗が1件でもあればすべて新規リグレッションとして報告する。

## 禁止事項

- `--test-threads` 指定なしのフルスイート実行（FSレース + OOM 既往）
- ベースライン5件を「直った/壊れた」と誤報告すること
