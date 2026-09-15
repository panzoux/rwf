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

## Phase 7 番号再割当の経緯（2026-09-15 ROADMAP.md から移動）

> **番号再割当について（履歴）**:
> - 2026-07-02: 旧表の 7.1(Undo)/7.2(Leap) が plan/ 配下の実ファイル名
>   （`7.6.transactional_rollback.md`・`7.8.leap_navigation.md`）およびコミット履歴（`feat(7.8)` = Leap）と
>   不一致だったため、ファイル名側を正として再割当。7.1・7.2 は欠番となった。
> - 2026-07-05: 欠番だった 7.1 に **Leap（完了済み、旧 7.8）** を、7.2 に **コマンドパレット（旧 Phase 8.7 から昇格）** を割当。
>   7.8 は欠番。**詳細ファイル名 `7.8.leap_navigation.md` とコミット履歴 `feat(7.8)` は歴史的経緯としてそのまま**（リネームしない）。

> **表の行順は実装優先順（上ほど先）**。番号（#列）は歴史的経緯で振られた識別子であり、
> 詳細ファイル名（`7.4.xxx.md` 等）・コミット履歴（`feat(7.x)`）と対応するため固定。
> 実行順の一次情報はこの表そのもの（旧「Phase 7 実装順序」節は本表に統合・廃止）。

## 初期の優先度・スケジュール感（2026-05 作成、2026-09-15 ROADMAP.md から移動）

```
Phase 1 (〜2週間)  → UIダイアログ完結、日常操作の安定化
Phase 2 (〜2週間)  → ナビゲーション強化
Phase 3 (〜3週間)  → ジョブ管理UI洗練
Phase 4 (〜3週間)  → ビューア完成（テキスト+Hex）
Phase 5 (〜2週間)  → アーカイブ拡張
Phase 6 (〜4週間)  → twf完全パリティ
Phase M (完了)     → 品質整備（機能凍結・解除済み。Phase 7 残タスクの前提）
Phase 7 (再開)     → 差別化機能
```

**Phase 1〜3完了**: 日常ユースケースでtwfと同等  
**Phase 4〜6完了**: 全機能でtwfパリティ達成  
**Phase 7**: rwf独自の強み確立

## ROADMAP外で実装済みの機能（2026-09-15 ROADMAP.md から移動）（2026-06-13〜07-02、要フェーズ整理）

- **シンボリックリンク/ジャンクション対応** — `LinkKind` enum、`symlink_metadata` による検出、一覧での `->` サフィックス表示、ファイル名バーの `name->target` 表示、File Information ダイアログの Type/Target 行（b414944〜fa2e5c1）
- **SuspendAndRun ジョブ** — ターミナルエディタ（vim 等）起動のための TUI サスペンド対応（ee3011b）
- **動的ヘルプビューア + `--export-config-files` + キーバインド衝突検出**（e406672、Phase 6.7 として完了扱い）
- **config status 表示の改善** — keybindings の「ファイル無し」とエラーの区別（6978949）
- **Input ダイアログのテキスト編集実装**（d87a383）
- **タスクパネル縮小順序の修正・スピナー共通化・LEAP バースピナー**（9e500c8, 13c7189, cad6bd7）

## ROADMAP 簡素化時に外した旧記述（2026-09-15、当時の文面をそのまま転記）

> Phase 1〜6・M・Future の旧セル全文、節の前置き、旧テスト戦略など。表の行は元の列構成のまま。

### 旧「(冒頭)」

**作成**: 2026-05-23  
**目標**: rwf を twf（C#プロトタイプ）と同等以上の機能・安定性に引き上げる  
**rwf の強み**: CJK文字表示、クロスプラットフォーム、メモリ効率、型安全性  
**現状完成度**: 約72%（コアロジック充実、UIダイアログ・ビューア系が主な不足）

### 旧「凡例」

- `[-]` 不採用（設計判断により実装しない。理由は詳細欄／詳細ファイル参照）

### 旧「Phase 1 — UIダイアログ穴埋め（バックエンド済み、UI未実装）」

**推定規模**: 各500〜800行  
**リスク**: 低（既存 DialogStack と同じパターン）

### 旧「Phase 2 — ジャンプ・ナビゲーション」

**推定規模**: 各800〜1200行  
**リスク**: 中（再帰検索の非同期キャンセル処理）

### 旧「Phase 3 — ジョブ管理UI」

> 詳細仕様は [plan_job_dialog.md](plan_job_dialog.md) を参照（推定60〜86時間）。
**テスト**: ジョブ状態遷移の単体テスト + UIレンダリングのスナップショットテスト

### 旧「Phase 4 — テキスト/バイナリビューア」

> モデル層 ([`model/viewer.rs`](../rwf-lib/src/model/viewer.rs)) は実装済み。  
> `ViewerMode::Text` と `ViewerMode::Hex` の両方がある（Hexも自前実装済み）。  
> TWFも同様に自前実装。不足はTUIレンダリング層とエンコーディング実装。
| 4.2 | **Hex/バイナリビューア TUI ウィジェット** | `[x]` | `get_hex_bytes_vec()` + `hex_row_spans()`（rwf-bin）でオフセット・ASCII表示（旧 `get_hex_line()` は 7.3b で未使用と判明し削除） |
| 4.3 | **大容量ファイル対応** | `[x]` | `memmap2` によるメモリマップ + `LineIndex` バックグラウンドインデックス（ファイル全体をRAMに乗せない） |
| 4.4 | **エンコーディング実装補完** | `[x]` | Shift-JIS/EUC-JP を `encoding_rs` クレートで完全実装済み |
| 4.5 | **エンコーディング自動検出** | `[x]` | BOM検出 + 日本語統計的検出を `TextEncoding::detect()` として実装済み |
**追加クレート候補**:
- `encoding_rs` — Shift-JIS/EUC-JP等のデコード（Mozilla製、クロスプラットフォーム）
**テスト**: エンコーディング検出ユニットテスト、大容量ファイルのメモリ使用量テスト、Hexレンダリング検証

### 旧「Phase 5 — アーカイブ拡張」

> 現状: `zip`クレートのみ。TWFは外部`7z.exe`（Windowsのみ）を使用。  
> rwfはクレートベースでクロスプラットフォーム対応を優先する。
| 5.1 | **7z サポート** | `[x]` | `sevenz-rust`（純Rust、win/mac/linux対応）`SevenZArchiveHandler` + `MultiFormatArchiveHandler` 実装済み、9テスト合格 |
| 5.2 | **TAR/TGZ サポート** | `[x]` | `tar` + `flate2` クレート。`TarArchiveHandler`実装済み（.tar/.tgz/.tar.gz）、10テスト合格 |
| 5.3 | **RAR サポート** | `[ ]` | `.rar` は認識済み（graceful error）。将来: `libarchive` クレート経由で実装予定（→ 5.6 参照） |
| 5.4 | **ISO サポート** | `[x]` | `iso9660` クレート（純Rust）でブラウズ・展開実装済み。作成不可（read-only） |
| 5.5 | **LZH サポート** | `[ ]` | `.lzh/.lha` は認識済み（graceful error）。将来: `libarchive` クレート経由で実装予定（→ 5.6 参照） |
| 5.6 | **libarchive 統合（RAR・LZH 他）** | `[ ]` | `compress-tools` クレート（`libarchive` ラッパー）で RAR/LZH/CAB 等を一括対応。libarchive がインストール済みの環境で有効化。将来の機能強化フェーズで実装 |
**テスト**: 実アーカイブファイルを使った統合テスト（各形式で作成→展開→内容確認）

### 旧「Phase 6 — twfパリティ完結（高度機能）」

> Phase 6完了でtwfとの完全パリティ達成。
| 6.1 | **設定システム・ペイン更新機構整備** | `[~]` | Layer 1更新機構(外部コマンド後の自動リフレッシュ)実装済み。ConfigManagerに extension_associations.json / custom_functions.json / context_menu.json パス追加。colors.json分離は未実装（後フェーズ） |
| 6.2 | **ファイルタイプ関連付け** | `[x]` | `ExtensionAssociation`構造体、`extension_associations.json`読み込み、Enter時の拡張子マッチ→外部コマンド実行。マクロ展開対応。AppState起動時ロード・Shift+Zでリロード。 |
| 6.3 | **カスタム関数システム** | `[x]` | `custom_functions.json`読み込み、Shift+T でカスタム関数選択ダイアログ表示・実行。インクリメンタルフィルタ、マクロ展開対応、PipeToAction対応。AppState起動時ロード・Shift+Zでリロード。 |
| 6.4 | **コンテキストメニューシステム** | `[x]` | `\`キー でコンテキストメニュー表示。デフォルト組み込みアクション(View/Copy/Move/Rename/Delete)+セパレータ対応。カスタム関数呼び出し対応(`ContextMenuAction::CustomFunction`)。上下ナビ（セパレータスキップ）実装。 |
| 6.5 | **カスタム関数メニューダイアログ** | `[x]` | `menu_xxx.json` 対応。メニュー型関数（`Menu` フィールド）を選択時に専用メニューダイアログを表示。`Action` フィールドでカスタム関数名またはビルトインアクション名を解決・実行。セパレータスキップ、文字キージャンプ対応。詳細: `plan/phase-6-6-custom-function-menus.md` |
| 6.6 | **ビューア大容量ファイルエンジン（LargeFileEngine 方式）** | `[x]` | mmap を廃止し `FileBytes::Seekable(SeekableFile)` へ移行。`File + Seek + Read` でページフォルト遅延を根絶。Hex検索もチャンク読みで対応。InMemoryしきい値は `viewer_large_file_threshold_mb`（デフォルト100MB）で設定可能。memmap2 依存を完全削除。 |
| 6.7 | **ヘルプ強化（実キーバインドビューア）** | `[x]` | `?`/F1 オンラインヘルプは修正済み（ハードコード表示）。設定変更を即反映する動的キーバインドビューアは未実装。Phase 6 機能セット確定後に対応 |

### 旧「Phase M — 品質整備フェーズ（**完了・凍結解除済み**。詳細: [quality_overhaul.md](quality_overhaul.md)）」

## Phase M — 品質整備フェーズ（**完了・凍結解除済み**。詳細: [quality_overhaul.md](quality_overhaul.md)）
> **🔓 機能開発凍結解除宣言（2026-07-13）**: M1〜M7 全タスク完了。検証一式（fmt / clippy --all-targets
> -D warnings / `cargo test -p rwf` 156 / `cargo test -p rwf --no-run` / `cargo test -p rwf-lib` 1044、
> 3042.45s）全緑。本計画全体を通じた挙動変更は M7 archive.rs の 1 件のみ（詳細は quality_overhaul.md の
> 「Phase M 完了サマリ」参照）。Phase 7 残タスクおよび Phase 8+ の機能開発に着手可能。
> **背景**: 複数 AI（kilo→antigravity→qwen→gemini→claude）を渡り歩いた開発による ad-hoc コード蓄積への対処。
> 本来の目的は一回きりの掃除ではなく「AI 主導開発でも品質が劣化しない仕組み」の構築。
> Phase 7 の残タスクに着手する前に本フェーズを完了させ、後続開発の負荷を下げた状態で Phase 7 以降を実施する
> という方針のもと、M 完了までは新機能実装を凍結していた（全タスクは挙動保存、M7 の archive.rs TODO 修正のみ例外）。
> 各タスク完了条件: fmt / clippy -D warnings / 全テスト緑 + コミット。

### 旧「Phase 7 — twf超え（rwf独自の強化）」

> CJK表示はすでに rwf の強み。さらに差別化できる機能。
> **着手条件: Phase M 完了 —満たされた（2026-07-13）。着手可能。**
### 推奨実装優先度（2026-07-05 番号再割当・状態更新）
| 7.4 | **バックグラウンド・ディレクトリサイズ計算** | `[ ]` | **Shift+S** で再帰的ディレクトリサイズを非同期計算。エントリごとにサイズを段階的に埋める。スピナー + Task pane ログ。詳細は [7.4.calculate_directory_size.md](7.4.calculate_directory_size.md) 参照 | ⭐⭐⭐⭐ | 2.5週間 |
| 7.5 | **バックグラウンドポーリング（Layer 2）** | `[ ]` | 可視エントリのメタデータ定期チェック（twf PerformSmartRefresh 相当）。間隔は config `polling_interval_ms`（1.4.2 で追加済み） | ⭐⭐⭐ | 2週間 |

### 旧「Future Enhancement Candidates (Phase 8+検討事項)」

> Phase 7 完了後に検討すべき高付加価値機能。
| **ユーザー定義マジックバイト表** | Phase 7.3 の内蔵シグネチャテーブル（手組み・約20〜30形式）でカバーしきれない形式向けに、`file_type_map.json` と同じ外部JSONパターンで拡張可能にする | Phase 8.7 |
| **libmagic 統合（オプション）** | `magic`クレート経由の完全なマジックバイト判定。libmagicがインストール済みの環境でのみ有効化（Phase 5.6 の compress-tools 同様の任意依存パターン）。Phase 7.3 では採用見送り（Windows非対応のため） | Phase 8.8 |
| **ファイル一覧ペインへの常時タイプ表示** | Phase 7.3 では File Information ダイアログでのオンデマンド表示のみ。常時アイコン/列表示は都度I/Oコストが発生するため別途検討 | Phase 8.9 |
| **migemo 統合方式の見直し（クレート優先 + 外部モジュール fallback）** | Rust の migemo クレートは Linux でのビルドに失敗することが判明。クロスプラットフォーム対応のクレートを優先利用しつつ、ビルド不可な環境（Linux/macOS）では TWF プロトタイプで動作実績のある `migemo.dll` 等の外部モジュール方式（辞書ファイル同梱）へフォールバックできないか検討する。TWF プロトタイプでは Windows/Linux 双方で動作実績あり | Phase 8.10 |
| **7-Zip 外部モジュール対応（アーカイブ形式拡張）** | 現行の 7z サポート（Phase 5.1、`sevenz-rust` クレート）は対応アーカイブ形式が限定的。TWF プロトタイプで動作実績のある 7-Zip 外部モジュール（`7z.dll`/`7za` 等）が利用可能な環境ではそちらを優先利用し、より多くの形式（RAR/LZH等、5.3・5.5・5.6 参照）に対応できないか検討する | Phase 8.11 |
> 旧 8.7 コマンドパレットは 2026-07-05 に Phase 7.2 へ昇格。マジックバイト判定（旧 8.7 再割当分）は
> 2026-07-18 に Phase 7.3 スマート・ファイルオープナーへ統合済み（[7.3.smart_file_opener.md](7.3.smart_file_opener.md)）。
> 上記 8.7〜8.9 はその設計で意図的にスコープ外とした派生候補。

### 旧「テスト戦略」

## テスト戦略
### フェーズ共通方針
ダイアログ系:    レンダリング出力の snapshot テスト
ファイル操作系:  assert_fs + tempfile による実FS上の統合テスト
非同期系:        tokio::test による並行処理・キャンセルテスト
エラー系:        権限不足・容量不足などの異常系パス
プロパティ:      proptest による状態遷移の網羅テスト（既存パターン踏襲）
### 現状テスト数: 1043件（rwf-lib 単体 + プロパティ + 統合、2026-07-02 実測）＋ rwf-bin UI テスト
> 実行時間の目安: rwf-lib 全件を `--test-threads=1` で約37分。通常はテスト名フィルタで対象のみ実行すること
> （実行手順の規約は `.claude/CLAUDE.local.md` を参照）。

### 旧「優先度・スケジュール感」

## 優先度・スケジュール感

### 旧「ROADMAP外で実装済みの機能（2026-06-13〜07-02、要フェーズ整理）」

## ROADMAP外で実装済みの機能（2026-06-13〜07-02、要フェーズ整理）

### 旧「実装内訳アーカイブ」

## 実装内訳アーカイブ
Phase 1〜4 の各タスクの実装内訳（コミット時点の詳細メモ）は
へ移動した（2026-07-18、ROADMAP.md 肥大化対策）。

### 旧「備考」

## 備考
- テスト実行の規約（OOM 対策・推奨コマンド）は `.claude/CLAUDE.local.md` の「Test Suite Status」を参照。
