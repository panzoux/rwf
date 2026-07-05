# M4 引き継ぎ — model/dialog.rs バリアント struct 化 + input ハンドラ抽出

先に `M_handoff_common.md` を読むこと。工数: **大**(5 セッション)。オーケストレータ: **sonnet**。

## ゴール

1. `DialogContent` の **29 バリアント**それぞれのフィールド群を専用 struct に畳む
   (`DialogContent::Sort { .. }` → `DialogContent::Sort(SortDialog)`)。
2. 共通 UI 状態(カーソル・スクロール・ボタンフォーカス・入力バッファ)は `DialogUiState` struct として各ダイアログ struct に埋め込む。
3. 各 struct に `new()`(必須引数のみ、UI 状態はデフォルト)を用意し、生成側の手動フィールドセットを全置換。
4. **M3 からの宿題**: struct 化完了後、`rwf-bin/src/ui/dialog/mod.rs`(現 2,024 行)の
   `handle_dialog_input` の各 match 腕の本体を各ダイアログファイルの `handle_input(&mut FooDialog, key, ...)` へ移動し、mod.rs を dispatch のみ(~100 行目標)にする。
5. `docs/DIALOG_DESIGN_SPEC.md` を新構造に更新。

## 確定済み設計(再設計禁止)

- `DialogContent` **enum 自体は維持**(Transition dispatch と噛み合っているため)。`rwf-lib/src/model/dialog/` をディレクトリ化し、`mod.rs` に enum と再エクスポート、バリアント struct は 1 ダイアログ 1 ファイル。
- `Dialog<T>` のようなジェネリック化は**やらない**。29 種の UI 状態は微妙に異なる — 共通部分だけ `DialogUiState` に入れ、固有フィールドは各 struct に残す。
- `DialogUiState` に入れるのは「複数ダイアログが同名で持つ UI 状態」のみ(候補: `selected_index`/`scroll_offset`/`focused_button` 系。実フィールド名は S1 で実測して確定し、この下の欄に記録する)。
- serde 属性(PascalCase)を持つ型が含まれる場合、リネームせずそのまま移す(config 互換を壊さない)。

### S1 で確定した DialogUiState 定義(S1 実施時に記入)

```
(S1 で記入)
```

## 並列化の禁止(重要 — 計画からの変更点)

M3 と違い、M4 の変更は `model/dialog.rs`(enum)・`state.rs`(生成箇所)・`ui/dialog/mod.rs`(match)という**共有ファイルに集中**する。並列 haiku は必ず衝突するので**直列バッチ**で行う:
haiku subagent を 1 体ずつ順番に投入(1 体 = 3〜5 バリアント、バリアントごとにコミット)。
各エージェント完了時に sonnet が diff 監査(非デフォルト値の欠落チェック)→ 次を投入。

## バリアント一覧(処理順 = 単純→複雑。チェックボックスで進捗管理)

単純(フィールド少・生成箇所少):
- [ ] Version / [ ] Error / [ ] Progress / [ ] Confirmation / [ ] DeleteConfirm
- [ ] ExtractionConfirm / [ ] CloseTabWithActiveJob / [ ] HistoryDialog / [ ] DriveSelection
- [ ] ContextMenu / [ ] TabSelector / [ ] RegisteredFolderSelector
中程度:
- [ ] Input / [ ] Help / [ ] SortDialog / [ ] FileMask / [ ] WildcardMark / [ ] SimpleRename
- [ ] JumpToPath / [ ] JumpToFile / [ ] FileInfo / [ ] CustomFunctionSelector / [ ] CustomFunctionMenu
複雑(フィールド多・入力処理重い — sonnet が直接担当):
- [ ] JobManager / [ ] PatternRename / [ ] ComparisonView / [ ] SplitJoinDialog
- [ ] Compression / [ ] FileConflict

## セッション分割

### S1(sonnet 単独): 設計確定 + テンプレート 2 件
1. `model/dialog/` ディレクトリ化(enum は mod.rs へ move-only。この時点で 1 コミット)。
2. 各バリアントの生成箇所を `Grep` で棚卸しし、`DialogUiState` のフィールドを実測で確定 → 本ファイルに記録。
3. テンプレートとして **SortDialog**(中程度)と **Version**(単純)を struct 化:
   struct 定義 + `new()` + enum 変更 + 生成側置換 + rwf-bin 側 match 腕の型合わせ。各 1 コミット。
4. このコミット 2 件が haiku への見本。見本コミットのハッシュを本ファイルに記録。
- 完了条件: 検証一式緑(rwf-lib フルはバックグラウンド)、進捗記入、コミット。

### S2〜S3(sonnet + 直列 haiku): 単純・中程度バリアント
- S2: 単純 12 件。haiku 1 体 = 4 件 × 3 体を**直列**投入。
- S3: 中程度 11 件。haiku 1 体 = 3〜4 件 × 3 体を直列投入。
- haiku への指示テンプレ(必ず全文含める):
  「見本コミット <hash> の形式に従い、バリアント X を struct 化する。手順: (1) model/dialog/x.rs に struct 定義 + DialogUiState 埋め込み + new()、(2) enum バリアントを tuple 形式に変更、(3) 生成箇所(<Grep 結果を貼る>)を new() 呼びに置換、(4) rwf-bin の match 腕のパターンを合わせる。1 バリアント完結してから次へ。旧フィールドアクセスの残存を grep で確認。**非デフォルト値(Some(..)/true/数値)を絶対に落とさない**。cargo 実行禁止。完了したら変更ファイル一覧を報告」
- 各バリアント = 1 コミット(sonnet が diff 監査後にコミット)。
- 各セッション末: 検証一式(clippy + rwf テスト。rwf-lib フルは S3 末のみで可)。

### S4(sonnet 単独): 複雑 6 バリアント
FileConflict / Compression / JobManager / PatternRename / ComparisonView / SplitJoinDialog。
フィールドが多く入力処理と絡むため haiku に出さない。1 バリアント 1 コミット。
完了時点で 29/29。rwf-lib フルテスト(バックグラウンド)+ スナップショット差分ゼロ確認。

### S5(sonnet 単独): input ハンドラ抽出 + 仕上げ
1. `handle_dialog_input` の各 match 腕本体を各ダイアログファイルの `handle_input(&mut FooDialog, ...)` へ移動(数腕ずつコミット。mod.rs 目標 ~100 行)。
2. `docs/DIALOG_DESIGN_SPEC.md` を新構造へ更新。`docs/recipes/add-a-dialog.md` ドラフトも現構造に追従させる(確定は M7)。
3. `model/dialog.rs` の `#![allow(clippy::unwrap_used)]` が分割後どのファイルに残るべきか確認(unwrap 11 箇所の所在ファイルにのみ引き継ぎ、他は allow を付けない)。ratchet リスト(quality_overhaul.md)を更新。
4. 検証一式全緑 → ROADMAP の M4 を `[x]`、本ファイルの進捗欄を完了に。

## セッション開始プロンプト(コピペ用)

```
plan/M_handoff_common.md と plan/M4_handoff.md を読み、M4 のセッション S<N> を実施してください。
進捗は M4_handoff.md のチェックボックスが正です。完了条件を満たしたらチェックを更新してコミットしてください。
```
