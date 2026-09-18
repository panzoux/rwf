# rwf 強化ロードマップ

**目標**: rwf を twf（C#プロトタイプ）と同等以上の機能・安定性に引き上げ、さらに rwf 独自の強みを確立する
**現在**: Phase 7（残: 7.5 実機検証〔動作確認→計測→体感〕、7.24 画像プレビュー）— 最終更新 2026-09-17

> **この文書の書き方**: 各行は 1 行の概要とリンクだけにする。経緯・実装メモ・検証記録は
> 項目ごとの詳細ファイル（`plan/<番号>.<名前>.md`）へ書く。セッションの引き継ぎ・日誌はここに書かない
> （現状は git log と各詳細ファイル、教訓はメモリ）。

---

## 凡例

- `[ ]` 未着手
- `[~]` 部分実装（UI要改善 or バックエンドのみ）
- `[x]` 完了
- `[-]` 延期（Phase 8+ 候補へ移動。[Future Enhancement Candidates](#future-enhancement-candidates-phase-8検討事項) 参照）
- 実装しないと決めたものは [不採用（Declined）](#不採用declined) 節へ

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

---

## Phase 2 — ジャンプ・ナビゲーション

| # | 機能 | 状態 | 詳細 | テスト方針 |
|---|------|------|------|-----------|
| 2.1 | **Jump to Path ダイアログ** | `[x]` | 複数キーワードAND絞り込み、非同期補完 | パス補完・AND検索のユニットテスト |
| 2.2 | **Jump to File ダイアログ** | `[x]` | 再帰検索、ignoreリスト対応 | 実FS上の統合テスト (tempfile) |

---

## Phase 3 — ジョブ管理UI

> 詳細仕様は [plan_job_dialog.md](plan_job_dialog.md) を参照。

| # | 機能 | 状態 | 詳細 |
|---|------|------|------|
| 3.1 | **タスクパネル** | `[x]` | 折り畳み/展開、ログ、スピナーアニメーション |
| 3.2 | **ジョブマネージャダイアログ** | `[x]` | 進捗表示、キャンセル操作、表示内容の洗練 |
| 3.3 | **タブのビジーインジケーター** | `[x]` | アクティブジョブ時のスピナー（TabBarView連携） |

---

## Phase 4 — テキスト/バイナリビューア

| # | 機能 | 状態 | 詳細 |
|---|------|------|------|
| 4.1 | **テキストビューア TUI ウィジェット** | `[x]` | スクロール、行番号、検索ハイライト |
| 4.2 | **Hex/バイナリビューア TUI ウィジェット** | `[x]` | `get_hex_bytes_vec()` + `hex_row_spans()`（rwf-bin）でオフセット・ASCII表示 |
| 4.3 | **大容量ファイル対応** | `[x]` | バックグラウンド `LineIndex`（ファイル全体をRAMに乗せない。mmap は 6.6 で廃止） |
| 4.4 | **エンコーディング実装補完** | `[x]` | Shift-JIS/EUC-JP を `encoding_rs` クレートで実装 |
| 4.5 | **エンコーディング自動検出** | `[x]` | BOM検出 + 日本語統計的検出（`TextEncoding::detect()`） |
| 4.6 | **サイドバイサイドビューアモード** | `[x]` | `v`=フルスクリーン、`V`=サイドバイサイド、Tab/Shift+Tab フォーカス移動 ([詳細](4.6.side-by-side_viewer_mode.md)) |

---

## Phase 5 — アーカイブ拡張

> rwf はクレートベースでクロスプラットフォーム対応を優先する（TWF は外部 `7z.exe`、Windows のみ）。

| # | 機能 | 状態 | クレート/方針 |
|---|------|------|-------------|
| 5.1 | **7z サポート** | `[x]` | `sevenz-rust`（純Rust）`SevenZArchiveHandler` + `MultiFormatArchiveHandler` |
| 5.2 | **TAR/TGZ サポート** | `[x]` | `tar` + `flate2`。`TarArchiveHandler`（.tar/.tgz/.tar.gz） |
| 5.3 | **RAR サポート** | `[ ]` | `.rar` は認識済み（graceful error）。5.6 経由で実装予定 |
| 5.4 | **ISO サポート** | `[x]` | `iso9660` クレートでブラウズ・展開（read-only） |
| 5.5 | **LZH サポート** | `[ ]` | `.lzh/.lha` は認識済み（graceful error）。5.6 経由で実装予定 |
| 5.6 | **libarchive 統合（RAR・LZH 他）** | `[ ]` | `compress-tools` クレート（`libarchive` ラッパー）。libarchive がある環境で有効化 |

---

## Phase 6 — twfパリティ完結（高度機能）

| # | 機能 | 状態 | 詳細 |
|---|------|------|------|
| 6.1 | **設定システム・ペイン更新機構整備** | `[~]` | Layer 1 更新機構・設定ファイル群のパス整備済み。colors.json 分離のみ未実装（[設計](6.1.config_system_design.REVISED.md)） |
| 6.2 | **ファイルタイプ関連付け** | `[x]` | `extension_associations.json` による Enter 時の外部コマンド実行、マクロ展開、Shift+Z リロード |
| 6.3 | **カスタム関数システム** | `[x]` | `custom_functions.json`、Shift+T 選択ダイアログ、PipeToAction 対応 |
| 6.4 | **コンテキストメニューシステム** | `[x]` | `\` キーで組み込みアクション＋カスタム関数のメニュー |
| 6.5 | **カスタム関数メニューダイアログ** | `[x]` | `menu_xxx.json` のメニュー型関数（[詳細](phase-6-6-custom-function-menus.md)） |
| 6.6 | **ビューア大容量ファイルエンジン（LargeFileEngine 方式）** | `[x]` | mmap を廃止し `SeekableFile` へ移行、しきい値 `viewer_large_file_threshold_mb`（[詳細](6.7.viewer-large-file-engine.md)） |
| 6.7 | **ヘルプ強化（実キーバインドビューア）** | `[x]` | 動的ヘルプビューア + `--export-config-files` + キーバインド衝突検出 |

---

## Phase M — 品質整備（完了 2026-07-13）

> AI 主導開発でも品質が劣化しない仕組みの構築。全タスク挙動保存（M7 の archive.rs 修正のみ例外）。
> 詳細・完了サマリ・各行の旧メモ: [quality_overhaul.md](quality_overhaul.md)

| # | タスク | 状態 | 概要 |
|---|---|---|---|
| M1 | ガードレール導入 | `[x]` | rustfmt / clippy.toml / workspace lints（unsafe_code・unwrap_used deny）/ CI fmt チェック |
| M2 | 共有部品・ドキュメント基盤 | `[x]` | `test_utils` fixture、dialog `common.rs`、CLAUDE.md・ARCHITECTURE・TESTING・recipes |
| M3 | dialog/mod.rs 分割 | `[x]` | insta スナップショット安全網 → ダイアログ単位 17 ファイルへ move-only 分割 |
| M4 | model/dialog.rs 分割 | `[x]` | 全 29 バリアントを struct 化、入力処理を各ダイアログへ（[M4_handoff.md](M4_handoff.md)） |
| M5 | state.rs 分割 | `[x]` | `state/handlers/` へ move-only 分割（[M5_handoff.md](M5_handoff.md)） |
| M6 | unwrap/clone 監査 | `[x]` | 非テスト unwrap 全撤去、ホットパス clone 7 箇所を借用化（[M6_handoff.md](M6_handoff.md)） |
| M7 | 仕上げ | `[x]` | recipes 確定・rustdoc・UI スモークテスト・凍結解除（[M7_handoff.md](M7_handoff.md)） |

---

## Phase 7 — twf超え（rwf独自の強化）

> 番号は識別子（詳細ファイル名・コミット `feat(7.x)` と対応）なので振り直さない。7.8 は欠番、
> 7.9 は不採用節へ。番号の経緯は [docs/history/roadmap_implementation_notes.md](../docs/history/roadmap_implementation_notes.md)。

| # | 機能 | 状態 | 概要 | 詳細 |
|---|------|------|------|------|
| 7.1 | **Leap ナビゲーション** | `[x]` | F3 で起動する AND セグメント＋prefix/substring/Migemo フィルタ移動 | [7.8.leap_navigation.md](7.8.leap_navigation.md) |
| 7.2 | **コマンドパレット** | `[-]` | ヘルプビューアの検索結果から Enter でアクションを直接実行 | [7.2.command_palette.md](7.2.command_palette.md) |
| 7.3 | **スマート・ファイルオープナー** | `[x]` | マジックバイト判定・Open With ピッカー・検出タイプ優先の関連付け解決（7.3b 含む） | [7.3.smart_file_opener.md](7.3.smart_file_opener.md) |
| 7.4 | **バックグラウンド・ディレクトリサイズ計算** | `[-]` | Shift+S で再帰サイズを非同期計算し段階的に表示 | [7.4.calculate_directory_size.md](7.4.calculate_directory_size.md) |
| 7.5 | **バックグラウンドポーリング（Layer 2）** | `[~]` | アクティブタブの可視ペインをバックグラウンド再読込（ドライブ単位のバックオフ・自動停止）。実装済み、§4 実機検証待ち | [7.5.background_polling.md](7.5.background_polling.md) · [ARCHITECTURE.md（ペイン更新機構）](../docs/ARCHITECTURE.md) |
| 7.5b | **ネイティブ FS 監視をポーリングのトリガーに** | `[-]` | 必要が生じるまで実装しない（2026-09-17 決定）。ポーリング方式としての 7.5 設計は完成形 | [7.5b.native_fs_watcher.md](7.5b.native_fs_watcher.md) |
| 7.6 | **Undo/Redo（トランザクション・ロールバック）** | `[x]` | 操作を Operation Record 化、`Alt+o` の Operation Report から LIFO で Undo/Redo | [UI](7.6.operation_report_ui.md) · [力学](7.6.transactional_rollback.md) |
| 7.7 | **スマート・トラッシュ** | `[x]` | OS ゴミ箱への移動・復元・空にする・一覧 UI | [7.7.smart_trash.md](7.7.smart_trash.md) |
| 7.10 | **SSH/SFTP対応** | `[-]` | リモートファイルシステム（大規模追加） | — |
| 7.11 | **属性/タイムスタンプ変更** | `[x]` | `Ctrl+a` で属性・パーミッション・タイムスタンプを一括編集 | [7.6.attribute_timestamp_edit.md](7.6.attribute_timestamp_edit.md) |
| 7.12 | **Create Link / Create File** | `[x]` | `;` でリンク/ジャンクション、`N` で空ファイル作成（Job 化で Undo 対象） | [7.6.create_link_file.md](7.6.create_link_file.md) |
| 7.13 | **ゴミ箱の自動削除** | `[-]` | `TrashConfig.auto_empty_days` による期限切れアイテムのパージ | [7.13.trash_auto_purge.md](7.13.trash_auto_purge.md) |
| 7.14 | **fish シェル風インライン自動補完** | `[-]` | 汎用 `InputDialog` に薄字の先読み補完 | [7.14.inline_autocomplete.md](7.14.inline_autocomplete.md) |
| 7.15 | **デバッグレポート機能（診断セッション）** | `[x]` | `F12` 診断セッション・`F11` スナップショット・診断バンドル出力 | [7.15.diagnostic_report.md](7.15.diagnostic_report.md) · [DIAGNOSTIC_BUNDLES.md](../docs/DIAGNOSTIC_BUNDLES.md) |
| 7.16 | **診断: JobProgress の記録（オプトイン）** | `[-]` | `Diagnostics.CaptureJobProgress` で進捗イベントを間引き記録 | [7.15.diagnostic_report.md](7.15.diagnostic_report.md) |
| 7.17 | **複数行テキスト入力ダイアログ** | `[x]` | ソフトラップ＋`↵` マーカー、`Ctrl+S` で確定 | [7.17.multiline_text_input.md](7.17.multiline_text_input.md) |
| 7.18 | **Linux 移植バグ** | `[x]` | Unix の fallback ゴミ箱と create_link スナップショットの移植性（Linux CI 緑） | [7.18.linux_port_bugs.md](7.18.linux_port_bugs.md) |
| 7.19 | **診断: スナップショットへの `viewer_anchor_pane` 記録** | `[x]` | SideBySide のアンカーペインを state projection に追加 | [7.15.diagnostic_report.md](7.15.diagnostic_report.md) |
| 7.20 | **クリップメニューの命名整理とメニュー説明表示** | `[x]` | ラベルを 5 行に改名、説明文の 2 カラム表示 | [7.20.clip_menu_naming.md](7.20.clip_menu_naming.md) |
| 7.21 | **メインスレッド上の同期 I/O 監査** | `[x]` | 描画パス・状態遷移から FS 呼び出しを排除し契約テストで固定 | [7.21.main_thread_io_audit.md](7.21.main_thread_io_audit.md) |
| 7.22 | **起動時ジョブの可視化とエラーダイアログの情報設計** | `[x]` | 読み込みのスピナー表示・構造化エラー種別・読み込み失敗ダイアログ | [7.22.startup_job_visibility.md](7.22.startup_job_visibility.md) |
| 7.23 | **メインスレッド非ブロッキング化とクリップボード経路の修正** | `[x]` | ジョブ投入口の一本化・セッション復元の FS 排除・ビジーループ解消 | [7.23.main_thread_nonblocking.md](7.23.main_thread_nonblocking.md) |
| 7.24 | **画像プレビュー・動画サムネイル** | `[ ]` | ffmpeg から生 RGB を受け半ブロックで描画（Rust クレート追加ゼロ案、zrr 由来） | [7.24.media_preview.md](7.24.media_preview.md) |
| 7.25 | **ペイン幅の変更（キー操作）** | `[~]` | `Ctrl+Left/Right` で境界を 2 列ずつ移動・`Ctrl+\|` で均等に戻す（タブ切替から付け替え）、SideBySide にも適用。実装済み、T8 実機確認待ち | [7.25.pane_width_resize.md](7.25.pane_width_resize.md) |

---

## Future Enhancement Candidates (Phase 8+検討事項)

> Phase 7 完了後に検討する候補。Phase 7 から延期した項目（`[-]`）も含む。

| 機能 | 詳細 | 可能性 |
|------|------|--------|
| **コマンドパレット** | ヘルプビューアから Enter で直接実行。コンテキスト絞り込みの設計が未決（[詳細](7.2.command_palette.md)） | 旧 7.2 |
| **バックグラウンド・ディレクトリサイズ計算** | Shift+S の非同期再帰サイズ計算（[詳細](7.4.calculate_directory_size.md)） | 旧 7.4 |
| **SSH/SFTP対応** | リモートファイルシステム | 旧 7.10 |
| **ネイティブ FS 監視をポーリングのトリガーに** | ローカル固定ドライブで `notify` の通知を「今すぐ再読込」の合図に使う。7.5 の計測・体感で遅延や負荷が問題になった時に再検討（[詳細](7.5b.native_fs_watcher.md)） | 旧 7.5b |
| **ゴミ箱の自動削除** | トリガー設計・fallback 側の age-filter・内蔵化の要否が未決（[詳細](7.13.trash_auto_purge.md)） | 旧 7.13 |
| **fish シェル風インライン自動補完** | 汎用 `InputDialog` の先読み補完（[詳細](7.14.inline_autocomplete.md)） | 旧 7.14 |
| **診断: JobProgress のオプトイン記録** | `AtomicU64` 1 個で間引く設計（[詳細](7.15.diagnostic_report.md)） | 旧 7.16 |
| **ディスク使用量可視化（グラフ）** | サイズ計算結果を円グラフ/棒グラフで表示。ncdu 風のビジュアル分析 | Phase 8.1 |
| **永続サイズキャッシュ** | `~/.rwf/size_cache.json` に計算結果を保存。ディレクトリ mtime で無効化判定 | Phase 8.2 |
| **動的ペイン幅調整（マウス）** | 境界のドラッグで幅を変更。キー操作分は 7.25 へ前倒し（[詳細](7.25.pane_width_resize.md)） | Phase 8.3 |
| **Escape キャンセル** | バックグラウンドジョブ実行中に Escape で即座にキャンセル | Phase 8.4 |
| **Git ステータス表示** | ペイン内で Git ファイル状態（modified/staged等）を色分け表示 | Phase 8.5 |
| **Registered Folder へのコピー/移動** | **CopyToRegisteredFolder** / **MoveToRegisteredFolder**。大量の登録フォルダから高速に絞り込み・選択して整理する機能 | Phase 8.6 |
| **ユーザー定義マジックバイト表** | Phase 7.3 の内蔵シグネチャテーブルでカバーしきれない形式向けに外部 JSON で拡張可能にする | Phase 8.7 |
| **libmagic 統合（オプション）** | `magic`クレート経由の完全なマジックバイト判定。libmagic がある環境でのみ有効化（5.6 と同じ任意依存パターン） | Phase 8.8 |
| **ファイル一覧ペインへの常時タイプ表示** | 7.3 は File Information でのオンデマンド表示のみ。常時表示は都度 I/O コストのため別途検討 | Phase 8.9 |
| **migemo 統合方式の見直し（クレート優先 + 外部モジュール fallback）** | migemo クレートは Linux ビルド失敗。ビルド不可環境では TWF で実績のある `migemo.dll` 等へフォールバックできないか検討 | Phase 8.10 |
| **7-Zip 外部モジュール対応（アーカイブ形式拡張）** | 7-Zip 外部モジュールがある環境ではそちらを優先し、RAR/LZH 等（5.3・5.5・5.6）に対応できないか検討 | Phase 8.11 |
| **`#![warn(missing_docs)]` 導入** | M7 で見送り（`model/dialog/` 約262項目・`rwf-bin/src/ui/` 約65項目が未着手） | Phase 8+ |

---

## 不採用（Declined）

> 検討したうえで**実装しない**と決めたもの。再検討するときは詳細の「理由」「再検討の条件」から読むこと。

| 案 | 理由（1 行） | 決定 | 詳細 |
|----|-------------|------|------|
| **7.9 シンタックスハイライト（`syntect`）** | ビューアのランダムアクセス設計と逐次・ステートフルなパーサが非適合。コード閲覧は外部ツールへ委譲（サイズは理由ではない） | 2026-08-29 | [7.9.syntax_highlighting.NOT_ADOPTED.md](7.9.syntax_highlighting.NOT_ADOPTED.md) |
| **FSWatcher（`notify` クレート）を更新の基盤にする** | ネットワークドライブ・仮想FSでイベントが欠落する。基盤は再読込（7.5）。再読込の**トリガー**としての案は 7.5b に記録（延期、必要が生じるまで実装しない） | 2026-05-24 | [ARCHITECTURE.md（ペイン更新機構）](../docs/ARCHITECTURE.md) · [7.5b](7.5b.native_fs_watcher.md) |
| **外部コマンドの影響範囲宣言（`refresh_after`）** | 定義漏れ・誤設定のリスク。完了後にアクティブペインを無条件リフレッシュする | 2026-05-24 | [ARCHITECTURE.md（ペイン更新機構）](../docs/ARCHITECTURE.md) |
| **JobProgress の常時記録（診断）** | 大きなコピー 1 回で数千レコード。オプトイン（旧 7.16）に限定 | 2026-08-12 | [7.15.diagnostic_report.md](7.15.diagnostic_report.md) |
| **libmagic の必須依存化（7.3）** | Windows 非対応。任意依存としては Phase 8.8 候補 | 2026-07-18 | [7.3.smart_file_opener.md](7.3.smart_file_opener.md) |
| **アーキテクチャ変更を伴う clone 削減（M6）** | 6 系統は churn に見合わない | 2026-07-12 | [M6_handoff.md](M6_handoff.md) |

---

## 関連文書

- テスト戦略・件数・実行時間: [docs/TESTING.md](../docs/TESTING.md)
- Phase 1〜4 の実装内訳・番号再割当の経緯・初期スケジュール: [docs/history/roadmap_implementation_notes.md](../docs/history/roadmap_implementation_notes.md)
- ペイン更新機構（Layer 1 / Layer 2）の設計決定: [docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md)
