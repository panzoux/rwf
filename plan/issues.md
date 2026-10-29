# 課題・未整理事項・調査事項

> 見つけた問題、まだ整理できていない事項、調べるべきことを置く場所。
> 実施すると決めたら Kanban（`kanban/`）にカードを作り、ここには 1 行のリンクだけ残す。
> 設計判断は各項目の詳細ファイル（`plan/<番号>.<名前>.md`）、計画は [ROADMAP.md](ROADMAP.md) へ。
> 解決したものは「解決済み」節へ移す（経緯が不要なら削除してよい）。

書式: `- **YYYY-MM-DD 見出し** — 事実 1〜2 行。関連: リンク / カード #N`

---

## 未解決

- **2026-09-18 Windows Terminal で `Ctrl+|` が届かない** — 7.25 の幅リセットキーが WT では
  rwf に届かない（診断バンドル `20260918-215728`）。関連: [7.25 §6](7.25.pane_width_resize.md) / カード #6

- **2026-09-20 タブが多いとタブバーに 2 個しか入らない** — ラベルのパス短縮長が 15 桁固定のため、
  1 タブ約 38 桁。96 桁端末に 7 タブだと 2 個＋`>` で残り 40 桁が空白のまま余る。
  タブ数に応じて短縮長を詰める案は未検討。関連: カード #32 / `rwf-bin/src/ui/tab_bar.rs`
- **2026-09-20 `● DIAG` バッジがタブバーの文字に重なる** — バッジはフレーム全体への
  オーバーレイ（`diag_badge.rs` の設計意図）なので、記録中は右端のタブ文字が隠れる。
  タブバー側で 14 桁を予約するかは未判断。関連: [7.15](7.15.diagnostic_report.md)
- **2026-09-26 herdr / alacritty で一覧・ダイアログが 1 桁ずれる（Windows 10 標準 ConPTY 由来）** — バンドル `20260926-115014`。
  曖昧幅説は否定（コンソール幅は全端末で `♪`=1 `…`=1、alacritty の単純出力も揃う）。
  再現: alacritty で起動時の Directory Unavailable ダイアログが `ネット ワーク名` と空白入りで出る。
  同じ rwf を WT で起動すると正常で、rwf のバッファ（スナップショット）にも空白はない。
  ratatui と同じ出力パターンを単体で流すと alacritty でも正常 → 端末に届く前の Windows 10 標準 ConPTY
  （alacritty・herdr が使う。WT は新しい自前 ConPTY）の再描画が原因と推定。
  検証 2026-09-26: alacritty の exe 横に NuGet `Microsoft.Windows.Console.ConPTY` 1.24.260710001 の
  `conpty.dll`+`OpenConsole.exe` を置いた複製で同時比較 → 標準 ConPTY は 6 回中 3 回空白、新 ConPTY は 4 回中 0 回。
  対処: alacritty はこの 2 ファイルを配置（rwf 側の変更不要）/ herdr は同機能なし（上流に要望）。関連: カード #36
- **2026-09-26 rwf-lib のテストが実 `%APPDATA%wfegistered_directory.json` を上書きする** —
  `test_register_folder_dialog_confirmation` などが `RegisterCurrentFolder` 経由で
  `RegisteredFolderManager::default_path()` に保存する。少なくとも 2026-09-09 から中身がテスト値
  （`/test/folder`）のまま。ユーザーの登録フォルダはそれ以前に消失している。

## 解決済み

- **2026-09-19 `kanban-md init` が `.gitignore` に非アンカーの `kanban/` を追記する** —
  リポジトリ規約（ディレクトリパターンはアンカー必須）に反するため `/kanban/` に手で修正済み。
  ボードを作り直すときは再発に注意。
