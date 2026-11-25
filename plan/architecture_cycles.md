# rwf-lib モジュール循環の解消

`deslop-please`（`.deslop-please.allow` に既存負債を凍結、CI の `architecture` ジョブ）が
検出した `rwf_lib` の module-cycle（15モジュール）を、層構造に直す計画。

## 目指す層（上が下に依存）
1. `state`, `input`
2. `job` 実行（`job_executor`）, `model::dialog` → トップレベル `dialog` へ
3. `config`, `config_layers`, `help_content`, `macro_expander`
4. 依存なし: `model` のデータ型, `session`, `volume_info`, キー型, `logging`

## 手順（効果の大きい順）— 1〜5 実施済み（記録参照）
1. `model/dialog/` を `model` から出す（逆流 config/help_content/macro_expander の主因、39ファイル）
2. `job` を型（下位）と実行（上位）に分ける（`model`→`job` 25本、`job_executor.rs` 4919行）
3. `config_layers` ⇔ `help_content`: `DEFAULT_*`/`BUILT_IN_MENUS` を依存なしの `defaults` へ、`help_content` は `ConfigLayers` を引数で受ける
4. `config`→`input`: キー/アクション型を下位の型モジュールへ
5. `diagnostics`/`logging`: スナップショットは呼び出し側で作って渡す

## 進め方
- 各手順は別コミット。型の移動のみ、挙動は変えない。`pub use` で外部パスを保つ。
- 手順ごとに `deslop-please --update-allow .deslop-please.allow` で許可リストを更新し、違反が単調に減ることを確認する。
- 依存数は grep の概算（テスト込み）。着手前に deslop-please の接続を全部取り直す。

## 記録
- 2026-11-25: 小さい循環2件（app⇔ui, log_layer⇔writer）は解消済み（acef11c）。
- 2026-11-25: module-cycle 0 件（15モジュール→解消）。型を下位へ移動して一方向化した:
  `model` へ DriveInfo/DriveType/ConflictPair/ConflictStatus/ArchiveFormat/ConfigOrigin/JobId、
  新モジュール `keybindings`(KeyBindings/Action)・`defaults`(埋め込み既定)・`custom_function`
  (CustomFunction ほか)・`macro_syntax`、`macro_expander` は `state/` 配下へ、open-with 解決は `state/file_open`、
  診断スナップショット生成は `state/diagnostic_capture`、`JobEvent` は `job`、`PipeToAction` は `pipe_to_action`。
  残る負債: wide-directory 7 件・long-file 7 件（`.deslop-please.allow`）。
