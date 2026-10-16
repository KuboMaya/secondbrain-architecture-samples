# SecondBrain — Architecture Portfolio Samples

既存の SaaS に頼らず、**プライバシー**と**オフライン耐性**を優先したローカルファーストな個人向け統合環境です。

クラウドにデータを預けるのではなく、メイン PC を信頼境界の中心に置き、スマホ PWA とは **Tailscale（P2P）経由のみ**で同期します。同期 API はインターネットに公開しません。

このリポジトリは、本番アプリ全体ではなく、**設計が伝わるコア実装の抜粋**です（転職・スカウト・技術レビュー用）。

> **開発状況:** 個人利用向けに **現在も開発中** です。同期・EventBus・オフライン経路などコアは動作していますが、機能追加と整理は続いています。製品デモではなく「設計と判断のサンプル」としてご覧ください。

> **開発ツール:** [Cursor](https://cursor.com/)（AI コーディング支援 IDE）を実装・リファクタに使っています。アーキテクチャ方針・ドメイン境界・テスト方針の判断は自分で行い、Cursor は実装速度と一貫性の維持に使っています。

---

## ひとことで言うと

**メイン PC をサーバー兼スマートディスプレイにし、スマホ PWA からも同じデータを扱えるローカルファースト個人 OS。**

| レイヤ | 技術 |
|--------|------|
| Desktop | Tauri v2 + React 19 |
| HTTP | Axum 0.8（同一プロセス） |
| DB | SQLite（WAL） |
| ドメイン連動 | `tokio::sync::broadcast` EventBus |
| オフライン | Dexie + Outbox |
| 同期 | Tailscale P2P → Axum + Bearer / LWW |
| 鍵配布 | X25519 wrap → WebCrypto（non-extractable） |
| AI / Voice | Gemini ストリーム → 句読点チャンク → ローカル TTS |

---

## 設計原則

1. **Local-first** — 正本は PC の SQLite。スマホは Dexie キャッシュ。失敗した変更は Outbox に積み、復帰後に flush。
2. **Event-driven Modular Monolith** — ドメイン同士は直接 import しない。連動は EventBus のみ（例: タスク完了 → ガチャ → SSE）。
3. **0ms 応答** — 自ドメインの DB 更新完了で即 HTTP 200。他ドメイン処理はバックグラウンド。
4. **多層防御の同期** — Tailscale 網内 + CGNAT 制限 + Bearer。衝突は `version` による LWW。
5. **ドメイン駆動 AI** — Gemini 呼び出しは `domains/ai` に閉じ、プロンプト／パースは各ドメインの `ai_handler`。

```
┌──────────────── メインPC ────────────────┐
│  Tauri + React（Desktop / Resident / …）   │
│       ↕ 同一プロセス                        │
│  Axum  ←── Tailscale 上のスマホ PWA         │
│       ↕                                     │
│  SQLite (WAL)  +  EventBus                  │
│       ↕                                     │
│  Gemini / ローカル TTS（WAV 再生キュー）     │
└─────────────────────────────────────────────┘
         ▲  Bearer + Tailscale のみ
┌────────┴────────┐
│ Dexie + Outbox  │
│ X25519 鍵受取   │
└─────────────────┘
```

---

## 読みどころ（推奨順）

| 見たいテーマ | ファイル |
|--------------|----------|
| EventBus / 0ms | `backend-core-samples/core/event_bus.rs`, `domains/task/services.rs`（`complete_task`） |
| 購読・Lagged 耐性 | `core/event_subscriber_runtime.rs`, `domains/reward/gacha_listener.rs` |
| Axum + CORS + エラー | `http/build_router.rs`, `core/error.rs`, `sync/access_middleware.rs` |
| sqlx CRUD + テスト | `domains/task/services.rs`, `domains/sleep/services.rs` |
| ガチャ曲線 / MC/DC | `domains/reward/gacha_math.rs` |
| オフライン + LWW | `frontend-sync-samples/dexie/offlineDb.ts`, `sync/lwwApply.ts` |
| 契約テストの厚み | `tests-samples/`（境界・EventBus 待ち・MC/DC） |

各ソース先頭に、本番からの抜粋であることと読みどころを短く書いています。

---

## リポジトリ構成

```
.
├── README.md
├── backend-core-samples/
│   ├── core/          # EventBus, error, crypto, WAL
│   ├── http/          # Axum router + CORS + middleware
│   ├── sync/          # Tailscale gate, LWW, evaluate
│   └── domains/       # task / sleep / reward / ai / knowledge / …
├── frontend-sync-samples/
│   ├── dexie/         # IndexedDB schema + Outbox
│   ├── api/           # offline-first fetch
│   ├── sync/          # LWW apply (+ unit test)
│   ├── crypto/        # X25519 unwrap
│   ├── layout/        # LayoutMode
│   └── tauri/         # secondary windows
├── voice-server-samples/
│   └── WavPlayQueue.cs
└── tests-samples/     # harness + conflict / bus integration
```

サーバー PC 向け管理 UI（WinForms）など運用専用コードは含めていません。

---

## スタック

- **Frontend:** React 19 / TypeScript / Vite / Dexie 4 / WebCrypto / PWA  
- **Backend:** Rust / Tauri 2 / Axum 0.8 / Tokio / sqlx + SQLite  
- **Sync / Security:** Tailscale · Bearer · X25519 wrap · version LWW  
- **AI / Voice:** Gemini · 句読点チャンク · .NET ローカル TTS  
- **Tooling:** Cursor + 手動のアーキテクチャ判断  

---

## 注意

- **単体で `cargo test` / `npm test` するクレートではありません。** 依存とモジュール境界は本番リポジトリ側にあります。
- 実 API キー・個人パス・実 DB は含めていません（プレースホルダーのみ）。
- 設計レビュー・技術面談向けの「読み物としてのコード」としてご利用ください。
