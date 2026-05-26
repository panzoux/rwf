# rwf 強化ロードマップ

**作成**: 2026-05-23  
**目標**: rwf を twf（C#プロトタイプ）と同等以上の機能・安定性に引き上げる  
**rwf の強み**: CJK文字表示、クロスプラットフォーム、メモリ効率、型安全性  
**現状完成度**: 約72%（コアロジック充実、UIダイアログ・ビューア系が主な不足）

---

## 凡例

- `[ ]` 未着手
- `[~]` 部分実装（UI要改善 or バックエンドのみ）
- `[x]` 完了

---

## Phase 1 — UIダイアログ穴埋め（バックエンド済み、UI未実装）

> バックエンドロジックが既存。UIラッパーを実装して基本操作を完結させる。

| # | 機能 | 状態 | テスト方針 |
|---|------|------|-----------|
| 1.1 | **ソートダイアログ** + 昇降順トグル | `[x]` | ダイアログレンダリング + ソート結果の単体テスト |
| 1.2 | **ファイルマスクダイアログ** | `[x]` | マスク適用前後のファイルリスト検証 |
| 1.3 | **ワイルドカードマーキングダイアログ** | `[x]` | MarkingModelとの統合テスト |
| 1.4 | **シンプルリネームダイアログ** | `[x]` | リネーム前後のファイル存在確認 (tempfile) |
| 1.4.1 | **外部コマンド完了後のペインリフレッシュ** | `[x]` | `state.rs` ExecuteCustomFunction 完了時にアクティブペインへ ReadDirectory 投入 |
| 1.4.2 | **ポーリング間隔の設定フィールド追加** | `[x]` | `config.rs` に `polling_interval_ms: u32`（デフォルト 1000）。Layer 2 ポーリング実装（Phase 7）に先行して設定スキーマを確定 |
| 1.5 | **衝突解決ダイアログ**（コピー/移動時） | `[x]` | 上書き/スキップ/名前変更/キャンセル各パスのテスト |
| 1.6 | **ヒストリダイアログ** | `[x]` | 履歴リストの表示・選択・ナビゲーション |
| 1.7 | **ドライブ変更ダイアログ** | `[x]` | OS ドライブ列挙バックエンド + 選択リスト UI（Windowsドライブレター・Unix マウントポイント対応） |
| 1.8 | **ファイル情報ダイアログ** | `[x]` | `Dialog::file_info()` コンストラクタ済み、UIレンダリングのみ（名前・パス・サイズ・日時・パーミッション） |
| 1.9 | **パターンリネームダイアログ** | `[x]` | パターン入力テキストボックス + ライブプレビュー一覧（`PatternRename { pattern, preview }` モデルあり） |
| 1.10 | **バージョン情報（タスクペイン出力）** | `[x]` | 起動時＋バックティク（`` ` ``）でタスクペインへシステム情報出力（ダイアログ不使用） |
| 1.11 | **ヘルプダイアログ** | `[x]` | `HelpContent` 実装済み、スクロール対応 UI・言語ローテーション・9テスト |
| 1.12 | **登録フォルダ選択ダイアログ** | `[x]` | `RegisteredFolderManager` 完全実装済み（load/save・env変数展開・フィルタ）、UI選択リスト + インクリメンタルフィルタ・11テスト |

**推定規模**: 各500〜800行  
**リスク**: 低（既存 DialogStack と同じパターン）

---

## Phase 2 — ジャンプ・ナビゲーション

| # | 機能 | 詳細 | テスト方針 |
|---|------|------|-----------|
| 2.1 | **Jump to Path ダイアログ** | 複数キーワードAND絞り込み、非同期補完 | パス補完・AND検索のユニットテスト |
| 2.2 | **Jump to File ダイアログ** | 再帰検索、ignoreリスト対応 | 実FS上の統合テスト (tempfile) |

**推定規模**: 各800〜1200行  
**リスク**: 中（再帰検索の非同期キャンセル処理）

---

## Phase 3 — ジョブ管理UI

> 詳細仕様は [plan_job_dialog.md](plan_job_dialog.md) を参照（推定60〜86時間）。

| # | 機能 | 状態 | 詳細 |
|---|------|------|------|
| 3.1 | **タスクパネル** | `[~]` | 折り畳み/展開、ログ、スピナーアニメーション |
| 3.2 | **ジョブマネージャダイアログ** | `[~]` | 進捗表示、キャンセル操作、表示内容の洗練 |
| 3.3 | **タブのビジーインジケーター** | `[ ]` | アクティブジョブ時のスピナー（TabBarView連携） |

**テスト**: ジョブ状態遷移の単体テスト + UIレンダリングのスナップショットテスト

---

## Phase 4 — テキスト/バイナリビューア

> モデル層 ([`model/viewer.rs`](../rwf-lib/src/model/viewer.rs)) は実装済み。  
> `ViewerMode::Text` と `ViewerMode::Hex` の両方がある（Hexも自前実装済み）。  
> TWFも同様に自前実装。不足はTUIレンダリング層とエンコーディング実装。

| # | 機能 | 状態 | 詳細 |
|---|------|------|------|
| 4.1 | **テキストビューア TUI ウィジェット** | `[~]` | スクロール、行番号、検索ハイライト |
| 4.2 | **Hex/バイナリビューア TUI ウィジェット** | `[~]` | `get_hex_line()` 使用、オフセット・ASCII表示 |
| 4.3 | **大容量ファイル対応** | `[ ]` | ストリーミング読み込み（全体をメモリに乗せない） |
| 4.4 | **エンコーディング実装補完** | `[~]` | Shift-JIS/EUC-JP のTODOを `encoding_rs` クレートで実装 |
| 4.5 | **エンコーディング自動検出** | `[ ]` | BOM検出 + 統計的検出（`chardet` 相当） |

**追加クレート候補**:
- `encoding_rs` — Shift-JIS/EUC-JP等のデコード（Mozilla製、クロスプラットフォーム）

**テスト**: エンコーディング検出ユニットテスト、大容量ファイルのメモリ使用量テスト、Hexレンダリング検証

---

## Phase 5 — アーカイブ拡張

> 現状: `zip`クレートのみ。TWFは外部`7z.exe`（Windowsのみ）を使用。  
> rwfはクレートベースでクロスプラットフォーム対応を優先する。

| # | 機能 | 状態 | クレート/方針 |
|---|------|------|-------------|
| 5.1 | **7z サポート** | `[ ]` | `sevenz-rust`（純Rust、win/mac/linux対応） |
| 5.2 | **TAR/TGZ サポート** | `[ ]` | `tar` + `flate2` クレート |
| 5.3 | **RAR サポート** | `[ ]` | `unrar`クレートまたは外部ツール連携（オプション） |

**テスト**: 実アーカイブファイルを使った統合テスト（各形式で作成→展開→内容確認）

---

## Phase 6 — twfパリティ完結（高度機能）

> Phase 6完了でtwfとの完全パリティ達成。

| # | 機能 | 状態 | 詳細 |
|---|------|------|------|
| 6.1 | **ペイン更新機構整備**（外部コマンド対応の基盤） | `[~]` | 設計決定事項セクション参照。Layer 1 の外部コマンド対応（1.4.1）は先行実装済み。Layer 2 ポーリングは 7.4 |
| 6.2 | **ファイルタイプ関連付け** | `[ ]` | 拡張子→外部ツール設定（`ExtensionAssociations`相当） |
| 6.3 | **カスタム関数システム** | `[ ]` | `CustomFunctionManager`相当、キー割り当て対応。refresh_after 宣言は不要（6.1 の設計による）。選択ダイアログUIは Phase 1.12 の登録フォルダと同パターンで追加予定 |
| 6.4 | **コンテキストメニューシステム** | `[ ]` | `menu_*.json`相当の設定ファイルベースメニュー |
| 6.5 | **ヘルプ強化（実キーバインドビューア）** | `[~]` | `?`/F1 オンラインヘルプは修正済み（ハードコード表示）。設定変更を即反映する動的キーバインドビューアは未実装。後で対応 |

---

## 設計決定事項 — ペイン更新機構

### 更新機構の2層モデル

ファイラーとして「本来あるべきファイルが見えない」状況は最大級のストレス。
効率より表示正確性を優先し、以下の2層で対応する。

#### Layer 1 — 操作起因の即時リフレッシュ

各操作完了後に、影響範囲に該当するペインへ `ReadDirectory` を投入する。

| 操作の種類 | アプリが知っていること | 更新方式 |
|-----------|-------------------|---------|
| Rename（単体） | from→to が完全既知 | インメモリ更新（フラッシュなし） |
| Copy / Move / Delete / Mkdir | 変化したディレクトリが既知 | ReadDirectory（同パスの全ペイン） |
| PatternRename | 変化したディレクトリが既知 | ReadDirectory（アクティブペイン） |
| ExtractArchive / CreateArchive | dest ディレクトリが既知 | ReadDirectory（同パスの全ペイン） |
| **外部コマンド（カスタム関数）** | **不明** | **アクティブペインを ReadDirectory** |

**外部コマンドの扱いに関する設計決定（2026-05-24）**:  
`refresh_after` のような影響範囲宣言をユーザーに求める案も検討したが採用しない。
定義漏れ・誤設定のリスクがあり、ユーザビリティを低下させる。
外部コマンドは「OS/外部プロセスによる変化」と同等に扱い、完了後に
アクティブペインを無条件リフレッシュする。効率は若干犠牲になるが、
表示正確性・公平性・設定の単純さを優先する。

#### Layer 2 — バックグラウンドポーリング（外部変化の追跡）

Layer 1 が捉えられない外部プロセス・他アプリによる変化を補完する。

- 方式: 表示中エントリのメタデータ（サイズ・更新日時）をタイマーで定期チェック（twf の `PerformSmartRefresh` 相当）
- 差分があれば ReadDirectory を投入
- 対象: ローカルドライブ、ネットワークドライブ、SDカード、クラウド同期ドライブ（Box・OneDrive等）
- twf での実績: 比較的遅いネットワーク/クラウドドライブでも安定動作を確認

**FSWatcher（notify クレート）は採用しない理由**:  
ネットワークドライブ・仮想FSでのイベント欠落が実用上の問題になりやすく、
ポーリングより信頼性が低い場面がある。ポーリングで十分な精度が得られる。

#### 例外 — アーカイブ仮想FS

- FSWatcher の対象外（実ファイルシステムではない）
- Layer 2 のポーリング対象外
- アーカイブ操作完了後の Layer 1 リフレッシュのみ適用

#### 実装優先度

| 機構 | フェーズ | 状態 |
|------|---------|------|
| Layer 1: 内部ジョブ後リフレッシュ | Phase 1〜5 | 既存 ✅ |
| Layer 1: 外部コマンド後アクティブペインリフレッシュ | Phase 6.2 | `[ ]` |
| Layer 2: バックグラウンドポーリング | Phase 7 | `[ ]` |

---

## Phase 7 — twf超え（rwf独自の強化）

> CJK表示はすでに rwf の強み。さらに差別化できる機能。

| # | 機能 | 詳細 |
|---|------|------|
| 7.1 | **シンタックスハイライト（ビューア）** | `syntect`クレートによるコードハイライト（twfにない） |
| 7.2 | **クロスプラットフォームアーカイブ** | クレートベースの7z/TAR（twfの`7z.exe`依存より優秀） |
| 7.3 | **CI/CDパイプライン** | GitHub Actions によるテスト自動化 |
| 7.4 | **バックグラウンドポーリング（Layer 2）** | 可視エントリのメタデータ定期チェック（twf PerformSmartRefresh 相当）。間隔は config `polling_interval_ms`（1.4.2 で追加済み） |
| 7.5 | **SSH/SFTP対応**（将来） | リモートファイルシステム（大規模追加） |

---

## テスト戦略

### フェーズ共通方針
```
ダイアログ系:    レンダリング出力の snapshot テスト
ファイル操作系:  assert_fs + tempfile による実FS上の統合テスト
非同期系:        tokio::test による並行処理・キャンセルテスト
エラー系:        権限不足・容量不足などの異常系パス
プロパティ:      proptest による状態遷移の網羅テスト（既存パターン踏襲）
```

### 現状テスト数: 815件（単体 + プロパティ + 統合）

---

## 優先度・スケジュール感

```
Phase 1 (〜2週間)  → UIダイアログ完結、日常操作の安定化
Phase 2 (〜2週間)  → ナビゲーション強化
Phase 3 (〜3週間)  → ジョブ管理UI洗練
Phase 4 (〜3週間)  → ビューア完成（テキスト+Hex）
Phase 5 (〜2週間)  → アーカイブ拡張
Phase 6 (〜4週間)  → twf完全パリティ
Phase 7 (随時)     → 差別化機能
```

**Phase 1〜3完了**: 日常ユースケースでtwfと同等  
**Phase 4〜6完了**: 全機能でtwfパリティ達成  
**Phase 7**: rwf独自の強み確立

---

## セッション再開時の確認事項

- 現在のフェーズ・タスク番号
- 最後に完了したタスク
- 残課題・ブロッカー

最終更新: 2026-05-26  
次の作業: Phase 1.11 ヘルプダイアログ

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

## 備考
- テストのOOM問題: `cargo test`でテストバイナリのメモリ不足が既存。`cargo build`は問題なし。
  テストは単体ファイル限定(`--lib --test`)での実行を推奨。
