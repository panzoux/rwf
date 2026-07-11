# M6 引き継ぎ — unwrap 監査(ratchet 完了)+ clone 監査

先に `M_handoff_common.md` を読むこと。工数: **中**(2 セッション)。オーケストレータ: **sonnet**。
前提: M4・M5 完了(allow の付け替えが済み、ratchet リストがファイル単位になっている)。

## ゴール

1. `#![allow(clippy::unwrap_used)]` を全撤去し、非テストコードの unwrap ゼロ。
   対象は **35 箇所**(M1 実測。quality_overhaul.md の ratchet リストが burn-down 表。
   M4/M5 の分割でファイルは変わっている可能性があるため、リストの最新状態を正とする)。
2. clone 監査: 対象を絞って修正(全 799 を潰さない)。

## 確定済み方針(再設計禁止)

- unwrap の 3 分類と処置:
  - (a) 不変条件により infallible → `expect("<なぜ infallible かの理由>")`。
    メッセージは「what failed」ではなく「なぜ起き得ないか」を書く(例: `expect("regex is compile-time constant")`)。
  - (b) 実際に失敗しうる → 既存のエラー経路(エラー Transition / Result 伝播)へ。
    **これは panic→エラー表示の挙動変更を含む。変更した箇所を本ファイル末尾の「挙動変更ログ」に必ず列挙。**
  - (c) lock poisoning 等「落ちてよい」系 → expect + メッセージ。
- `clippy::expect_used` は**導入しない**(expect + 理由メッセージが着地点)。
- clone の対象条件: (i) `Vec<FileEntry>` 等 O(エントリ数) の clone、(ii) キー入力毎に走るホットパス。
  小さい struct の clone は許容(CLAUDE.md に明記済みの方針)。clippy の clone 系 lint は導入しない。
- 修正手段の優先順: 借用化 > `std::mem::take` > `Arc` 化。`Arc` 化は共有先が本当に必要な場合のみ。

## セッション分割

### S1(sonnet 単独): unwrap 監査 — 35 箇所は少ないので subagent 不要
1. ratchet リストのモジュールを 1 つずつ処理: 全 unwrap を分類 → 修正 → `#![allow]` 削除 →
   clippy でそのモジュールが素通しになることを確認 → **1 モジュール = 1 コミット** → リスト消し込み。
2. 推奨順: 小さい順(job.rs / model/viewer.rs / pattern_rename.rs → logging.rs → ui/viewer.rs →
   job_executor.rs → macro_expander.rs → 旧 state.rs 系 → 旧 model/dialog.rs 系)。
3. (b) 分類でエラー経路の設計に迷ったら: 新しいエラー型を作らない。既存の
   `Transition` のエラー系 / `anyhow` 経路に乗せる。既存経路が本当に無い場合のみ
   expect + メッセージで暫定処置し、本ファイルに「M7 送り」と記録。
- 完了条件: 全 allow 撤去、clippy 緑、rwf テスト緑、rwf-lib フル(バックグラウンド)緑、コミット。

### S2(sonnet + Explore×haiku): clone 監査
1. Explore(haiku)2 体並列で候補抽出(読み取りのみ):
   (a) `.clone()` のうち型が `Vec<FileEntry>` / `Vec<String>`(一覧系)/ `PaneModel` / 大 struct のもの
   (b) キー入力処理〜update_state 経路(handlers/)内の clone
   出力形式: `file:line / 型 / 呼び出し文脈(ホットパスか)` の表。
2. sonnet が表をトリアージし、**上位 10〜20 箇所だけ**修正。1〜3 箇所 = 1 コミット。
3. 挙動保存なので既存テストが回帰網。борrow checker と格闘して 30 分超えるものは「見送り」として表に理由を記録(無理をしない — churn 回避が方針)。
4. 検証一式全緑 → ROADMAP の M6 を `[x]`。

## 進捗チェックボックス(ratchet 本体は quality_overhaul.md の表で消し込み)

- [x] S1 unwrap 全 35 箇所処置・allow 全撤去(2026-07-11 完了。9 モジュール全て `[x]` — quality_overhaul.md 参照。
      内訳: (a) infallible → `expect` = 24 箇所 / (c) lock poisoning → `expect` = 11 箇所 / (b) エラー伝播への変更 = 0 箇所。
      `cargo fmt --check` / `cargo clippy --all-targets -D warnings`(workspace 全体)/ `cargo test -p rwf`(145 件)/
      `cargo test -p rwf --no-run` / `cargo test -p rwf-lib`(1043 件、2187.76s)全緑。9 コミット、1 モジュール = 1 コミット)
- [ ] S2-1 clone 候補表(下の欄に貼る)
- [ ] S2-2 clone 修正(修正 __ 件 / 見送り __ 件)
- [ ] 全検証緑 + ROADMAP 更新

## clone 候補表(S2 で記入)

```
(S2 で記入)
```

## 挙動変更ログ(panic → エラー表示にした箇所。M7 のフェーズサマリに転記する)

- (S1)該当なし。35 箇所全てが (a) infallible または (c) lock poisoning に分類され、
  panic 経路自体は保存(unwrap → expect + 理由メッセージへの置換のみ)。既存のエラー Transition /
  Result 伝播へ切り替えた (b) 分類の箇所はゼロだった。

## セッション開始プロンプト(コピペ用)

```
plan/M_handoff_common.md と plan/M6_handoff.md を読み、M6 のセッション S<N> を実施してください。
burn-down は plan/quality_overhaul.md の ratchet リストが正。1 モジュール = 1 コミット。
(b) 分類(エラー伝播)の挙動変更は M6_handoff.md の挙動変更ログに必ず記録してください。
```
