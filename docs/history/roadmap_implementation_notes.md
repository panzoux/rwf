# ROADMAP 実装内訳アーカイブ

> 2026-07-18: [plan/ROADMAP.md](../../plan/ROADMAP.md) の末尾に蓄積していた各タスクの
> 実装内訳（"## X.Y 実装内訳" ブロック）をここへ移動。ROADMAP.md 側は前方参照用の
> 計画ドキュメントとして保ち、完了済みタスクの詳細はここに保存する。当時の内容をそのまま転記。

## 4.6 実装内訳（2026-05-30）
- `ViewerLayout` enum（`FullScreen` / `SideBySide`）を `model/ui.rs` に追加
- `LayoutState` に `viewer_layout: ViewerLayout`・`viewer_preferred_layout: ViewerLayout` フィールド追加
- `Transition::OpenSideBySideViewer { location }` — ファイルペインにフォーカスを残してビューア表示
- `Transition::ViewerSwitchLayout { layout }` — FullScreen ↔ SideBySide 切り替え（UIMode も同時に更新）
- `OpenTextViewer` / `OpenHexViewer` / `CloseViewer` に `viewer_layout` リセット処理を追加
- `app.rs` セクション 2.0: ビューアモード中の `v`/`V`/Tab 処理（SideBySide ↔ FullScreen サイクル）
- `app.rs` セクション 3.5: SideBySide 中の Tab/Shift+Tab でファイルペイン → ビューアへフォーカス移動
- `app.rs` セクション 3.6: 通常モード中の `v`（preferred layout で開く / FullScreen→閉じる / SideBySide→FullScreen）と `V`（SideBySide で開く / FullScreen→SideBySide / SideBySide→閉じる）
- `ui.rs` `render_ui()`: SideBySide レイアウト — 縦3段（tab bar / content / task panel）→ content を水平50/50分割。アクティブペインの反対側にビューア配置。ファイルペイン側は既存の path/volume/panes/pane-info/filename 構成を維持
- タスクパネルは常に画面下部に表示（ビューアに隠れない）
- `docs/rwf/keybindings.json` キーバインド記述更新（`V` → OpenSideBySideViewer、`Shift+V` 説明更新）

## 2.1 実装内訳（2026-05-26）
- `DialogContent::JumpToPath { query, cursor_pos, scroll_pos, candidates, suggestions, selected_index, search_root }` を dialog.rs に追加
- `Dialog::jump_to_path(search_root, candidates)` コンストラクタ追加
- `filter_jump_to_path_suggestions(candidates, query)` — スペース区切りトークンの AND 絞り込み関数（テスト可能）
- `collect_jump_candidates(state, root)` — ダイアログ開時に候補収集: カレントペイン dir → 登録フォルダ → ナビゲーション履歴 → 再帰ディスク探索（depth 3, max 100）
- `Transition::ShowJumpToPathDialog` — 候補収集してダイアログ push
- `Action::ShowJumpToPathDialog` + `J` キーバインド（twf の `Shift+J` 相当）
- `dialog/mod.rs`:
  - height: suggestions.min(10)+5, min 8; 80% height グループ; 70% width
  - `render_jump_to_path_dialog()`: 入力行（クエリ + ヒット数）→ 区切り線 → 候補リスト（スクロール対応）→ 区切り線 → フルパスプレビュー → ヒント行
  - input handler: ↑↓/j/k で選択移動、文字入力でリアルタイム AND フィルタ、Backspace/Ctrl+K でクエリ編集、Enter/Esc
  - process_dialog_confirmation: 選択パスへ ChangeLocation（フォールバック: クエリを直接パスとして解釈）
- `jump_to_path_tests.rs` — 11テスト: キーバインド(1)、ダイアログ開く(2)、初期状態(3)、フィルタロジック(5)
- 全11テスト合格

## 1.10 実装内訳（2026-05-26）
- モーダルダイアログ方式を廃止し、タスクペインへの出力方式を採用
- 出力フォーマット: `[System] RWF v{ver} | {os}/{arch} | Config: {config_path} | Log: {log_dir} | archives: ZIP | migemo: {status}`
- 起動時: `App::with_cwd_flag()` の既存スタートアップログを `build_version_info()` に置き換え
- バックティク（`` ` ``）: `Action::ShowVersionInfo` に割り当て（旧: `ShowContextMenu`）
  - `Action::ShowVersionInfo` を `Action` enum に追加
  - `action_to_transitions` では空ベクタを返す（app 層で処理）
  - `handle_key_event` で検出し `log_version_info()` を呼ぶ
- `App::build_version_info(state: &AppState) -> String` — `ConfigManager::new().config_path()` と `default_log_dir()` で実パスを取得
- `App::log_version_info(&mut self)` — task_panel.add_log へ出力

## 1.9 実装内訳（2026-05-25）
- `DialogContent::PatternRename` に `cursor_pos`, `scroll_pos`, `focused_field`, `preview_scroll` 追加
- `Dialog::pattern_rename()` コンストラクタでカーソル位置を pattern.len() に初期化
- `as_pattern_rename()` / `as_pattern_rename_mut()` に `..` を追加
- `DialogAction::PatternChanged` 追加（テキスト変更時にプレビュー再生成シグナル）
- `dialog/mod.rs`:
  - height: プレビュー行数 + 3（input+hint+status）、min 8、80% screen height
  - width: 60%（デフォルト）
  - `render_pattern_rename_dialog()`: Pattern行、シンタックスヒント行、プレビューリスト（変更=Yellow、未変更=DarkGray）、ステータス行
  - input handler: Tab(focus cycle 0→1→2)、Esc/Enter、PageUp/Down(preview scroll)、TextInput(textbox focused)
  - confirmation: PatternRename job (`JobKind::PatternRename { targets, pattern }`) を生成
- `app.rs`: `PatternChanged` アクション → `UpdatePatternRenamePattern` transition でプレビュー再生成
- `pattern_rename_dialog_tests.rs` 8テスト追加（キー, ダイアログ開く, タイトル, 初期値, 空ペイン時なし, プレビュー更新, 変換確認, マーク済みファイル）
- 全8テスト合格

## 1.7 実装内訳（2026-05-25）
- `DialogContent::DriveSelection` に `filter: String` 追加（インクリメンタル検索状態）
- `DriveInfo::display_label()` メソッド追加（ホーム/NWシェア/ローカルドライブ別フォーマット）
- `Dialog::drive_selection()` のタイトルを "Select Drive" に変更
- `state.rs` `ShowDriveChangeDialog` ハンドラを3段構成に拡張:
  1. ホームディレクトリ (`~ User Directory`)
  2. 両ペイン履歴からのNWシェアルート (`\\server\share` 形式で重複排除)
  3. OS ドライブ一覧
- `get_share_root_from_location()` ヘルパ関数追加
- `as_drive_selection()` / `as_drive_selection_mut()` に `filter` 戻り値追加
- `filter()` / `set_filter()` メソッドを `DriveSelection` に対応
- `dialog/mod.rs`:
  - height: エントリ数 + hint(1) + search(1)
  - width: 60 chars
  - `render_drive_selection_dialog()`: リスト表示、ヒント行、`/filter` 行
  - input handler: Up/Down/j/k/Home/End/Backspace/Ctrl+K/Enter/Esc/印刷文字
  - confirmation: フィルタ適用後の選択エントリへ `ChangeLocation`
- `context_menu_drive_selection_tests.rs` のパターンマッチ・タイトルを更新
- `drive_dialog_tests.rs` 11テスト追加（キー、display_label 5種、ダイアログ開く、タイトル、ホームエントリ、NWシェア、フィルタ初期値）
- 全11テスト合格

## 1.6 実装内訳（2026-05-25）
- `DialogContent::HistoryDialog { entries, selected_index, current_pos }` を dialog.rs に追加
- `Dialog::history_dialog(entries, current_pos)` コンストラクタ追加
- `model/navigation.rs` に `stack_and_pos()` / `jump_to_index()` メソッド追加
- `Transition::NavigateToHistoryIndex { pane, index }` を state.rs に追加、ハンドラ実装（キャッシュ対応）
- `Action::ShowHistoryDialog` 追加、`h` キーバインド
- `ShowHistoryDialog` ハンドラ: NavigationHistory スタック + 現在地をエントリ一覧に結合して開く
- `dialog/mod.rs` に height・render dispatch・`render_history_dialog()`（逆順表示、`>`カーソル、`*`現在地）を追加
- input handler: Up/Down/j/k/g/G/Enter/Esc
- confirmation handler: `NavigateToHistoryIndex` を返す
- 空履歴（エントリ1件以下）では開かない
- `history_tests.rs` 9テスト追加（キー、空履歴、ダイアログ開く、タイトル、エントリ内容、インデックス位置、ナビゲーション遷移、境界外、有効ジャンプ）
- 全9テスト合格

## 1.5 修正内訳（2026-05-25）
- `FileConflict` に `operation: String` フィールド追加（"Copy"/"Move" を格納）
- `Dialog::file_conflict()` に `op_name: &str` 引数追加、タイトルを動的生成
- `update_file_conflict_title()` を `operation` フィールド参照に変更
- `app.rs`: Copy/Move 判定して `op_name` を渡すよう修正
- `app.rs`: `ConfirmAll` ハンドラ追加（Shift+Enter が動作しなかった重大バグ修正）
- `dialog/mod.rs`: textbox フォーカス時の Tab `% 6` → `% 5`、BackTab wrap `5` → `4` バグ修正
- 11テスト追加（validate_filename × 5、Force/Skip/Cancel/Esc/ConfirmAll/Tab cycle）

## 1.2 修正内訳（2026-05-24）
- `raw_entries: Vec<FileEntry>` を PaneModel に追加（フィルタ適用前のマスタリスト）
- `apply_sort()` を `cmp_entries()` 自由関数化し、raw_entries も同時にソート
- `apply_current_filter()` を raw_entries から復元後フィルタ適用する方式に変更
- `SetFileMask` transition: `with_refresh()`（再読み込み）→ `with_ui_change()`（インメモリ）に修正
- ReadDirectory 完了時に `raw_entries` をセット、陳腐化チェックを `raw_entries` ベースに変更
- `*` キーを `WildcardMarking` → `FileMaskFilter` に変更
- パスラインにマスク表示 `[*.txt]` を追加（path_line.rs）
- `s` 単独キーバインドを削除（`s+n`等のシーケンスと競合していたバグを修正）
- search_filter_tests.rs の既存テストを新 FileMask ダイアログ仕様に更新

## DIALOG_DESIGN_SPEC.md 追記内訳 (v2.1)
- Appendix C に 3件の新規ミス事例追加（Block::title重複、行幅不揃い、ボタンParagraph着色）
- Part 8 追加: SortDialogの仕様（レイアウト、レンダリングルール、キーバインド、状態構造体）

## 1.4 実装内訳（2026-05-24）
- `DialogContent::SimpleRename { input, cursor_pos, scroll_pos, focused_field }` を dialog.rs に追加
- `Dialog::simple_rename(current_name)` コンストラクタ追加（cursor は文字数末尾）
- `Action::Rename` を `DialogContent::Input`（レガシー）→ `Dialog::simple_rename()` に変更
- `state.rs` の `title == "Rename"` レガシー分岐を削除
- `dialog/mod.rs` に height(5)・exact-height グループ・render dispatch・`render_simple_rename_dialog()` を追加
- input handler: Tab/Enter/Esc/TextInput（FileMask と同パターン）
- confirmation handler: `JobKind::Rename { from, to }` を返す
- `rename_tests.rs` 7テスト追加（キー、ダイアログオープン、タイトル、プレフィル、カーソル位置、空ペイン、ディレクトリ）
- 全7テスト合格
