# M7 引き継ぎ — 仕上げ(レシピ確定・rustdoc・スモークテスト・凍結解除)

先に `M_handoff_common.md` を読むこと。工数: **中**(2 セッション)。オーケストレータ: **sonnet**。
前提: M4〜M6 完了(最終構造が確定している)。

## ゴールと確定済み方針

1. `docs/recipes/add-a-dialog.md` / `add-a-transition.md` を最終構造で**確定**(M2 のドラフトを更新)。
   チェックリスト形式: 新ダイアログ = `model/dialog/xxx.rs`(struct + new)+ `ui/dialog/xxx.rs`
   (render + handle_input + スナップショットテスト)+ dispatch 2 箇所 + Transition。
   ルート CLAUDE.md からリンク。**このレシピが「AI の ad-hoc コード生成防止」の最終成果物 — 実際に手順通りに追えるかを既存ダイアログ 1 つで検証してから確定する。**
2. `backend/` `job/` `model/` の公開 API rustdoc。`#![warn(missing_docs)]` は全公開項目カバー後のみ導入(未達なら導入見送りを記録)。
3. `rwf-lib/src/backend/archive.rs` の TODO(タイムスタンプ処理)修正 —
   **Phase M 唯一の挙動変更**。単独コミット + テスト追加。(M1 の fmt で行番号は 222 からずれている。`Grep "TODO"` で特定。)
4. rwf-bin 未テスト UI へ TestBackend スモーク/スナップショット(panes / task_panel / viewer / tab_bar 優先)。
   基準は「render が panic しない + 代表状態のスナップショット」。全網羅不要。
5. ルート直下の `*_SUMMARY.md` / `BUGFIX_*.md` 類を `docs/history/` へ `git mv`(内容は変更しない)。
6. M6 の「挙動変更ログ」を ROADMAP の M6 行の注記に転記。
7. ROADMAP の Phase M を全完了マークし、**機能開発凍結解除を宣言**。CI 37 分問題は Phase 8+ 課題として記録のみ。

## セッション分割

### S1(sonnet + 並列 haiku 可): レシピ + rustdoc + md 整理
1. レシピ 2 本を確定(sonnet。既存ダイアログでの手順検証込み)。
2. rustdoc: haiku(general-purpose)3 体**並列可**(backend/ job/ model/ で 1 体ずつ —
   モジュールが分かれており衝突しない)。指示: 「公開 pub アイテムに /// を付ける。実装を変更しない。
   既存コメントを削除しない。1 ファイル完結。cargo 実行禁止」。sonnet が内容レビュー
   (機械生成の無内容 doc「Returns the x」を却下し、要点のみに直す)。
3. ルート md の `git mv` 整理。
- 各タスク 1 コミット。完了条件: clippy + rwf テスト緑、進捗記入。

### S2(sonnet + 並列 haiku 可): スモークテスト + TODO 修正 + 凍結解除
1. archive.rs TODO 修正(sonnet。挙動変更なので単独コミット + テスト。insta 差分が出たら
   この修正由来かを確認し、由来なら accept してよい — M フェーズ唯一の例外)。
2. UI スモークテスト: haiku 2〜3 体並列可(対象ファイルが独立)。M3 の snapshot_tests/ の
   既存ハーネスを見本に添付。
3. `#![warn(missing_docs)]` 導入判定 → 導入 or 見送り記録。
4. 検証一式全緑(rwf-lib フル含む)→ ROADMAP の M7 と Phase M 全体を `[x]`、
   **凍結解除を ROADMAP に宣言** → quality_overhaul.md に完了サマリ(挙動変更 2 件:
   M6 エラー伝播 + M7 archive.rs を必ず列挙)。
5. Phase 7 再開の次タスクは 7.2 コマンドパレット(ROADMAP 参照)。

## 進捗チェックボックス

- [x] S1-1 add-a-dialog.md 確定(手順検証済み。SortDialog の実装箇所と照合して
      state.rs→state/mod.rs + state/handlers/ui.rs の記述を修正)
- [x] S1-2 add-a-transition.md 確定(state.rs→state/mod.rs + state/handlers/*.rs
      の記述を修正、event_receiver.rs/job_executor.rs の記述は現状と一致を確認)
- [x] S1-3 rustdoc: backend / job / model(haiku×3並列。既存 docs でほぼ網羅済みと判明、
      未網羅の約50箇所を追加。find_match_ranges の誤解を招くdocを1件修正)
- [x] S1-4 ルート md 整理(git mv。BUGFIX_*/*_SUMMARY.md 11 ファイルを docs/history/ へ)
- [x] S2-1 archive.rs TODO 修正 + テスト(ZIP エントリの MS-DOS タイムスタンプを実抽出。
      `test_archive_browsing_extracts_stored_timestamp` 追加、39 archive テスト全緑)
- [x] S2-2 UI スモークテスト(panes / task_panel / viewer / tab_bar。TestBackend + 固定データで
      panic なし確認 + insta スナップショット1枚ずつ、計11テスト追加。`cargo test -p rwf` 156(旧145)全緑)
- [x] S2-3 missing_docs 判定(**見送り**。理由: S1 では backend/job/model(非dialog)のみ対象で
      262件の `model/dialog/` と 65件の `rwf-bin/src/ui/`、加えて `state/`/`input/`/`config.rs` 等
      未着手の公開項目が広範に残存。全公開項目カバーという前提条件に遠く未達のため導入見送り。
      Phase 8+ で rustdoc 網羅を継続タスクとして扱う)
- [ ] S2-4 全検証緑 + Phase M 完了宣言 + 凍結解除

## セッション開始プロンプト(コピペ用)

```
plan/M_handoff_common.md と plan/M7_handoff.md を読み、M7 のセッション S<N> を実施してください。
archive.rs TODO 修正以外は挙動保存です。完了時は ROADMAP の Phase M 完了宣言と凍結解除まで行ってください。
```
