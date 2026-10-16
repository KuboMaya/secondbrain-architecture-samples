# SecondBrain — Architecture Portfolio Samples

既存の SaaS に依存せず、**完全なプライバシー**と**オフライン耐性**を持つ、自分専用のローカルファースト統合環境を構築した個人プロジェクトです。

クラウド上に「第二の脳」を預けるのではなく、メイン PC を信頼境界の中心に置き、スマホ PWA からは Tailscale 上の暗号化トンネル経由でのみデータを扱います。同期 API をインターネットに公開しません。

本ディレクトリは、転職・スカウト向けに **設計思想が伝わるコアモジュールだけ** を抜粋したサンプル集です（アプリ全体の複製ではありません）。

> **ステータス:** 個人利用向けに **現在も開発中** です。コアの同期・EventBus・オフライン経路は動いていますが、機能追加・整理は継続しています。完成品の製品デモではなく、設計と判断のサンプルとしてご覧ください。

> **開発ツール:** 実装・リファクタ・設計整理には [Cursor](https://cursor.com/)（AI コーディング支援 IDE）を活用しています。アーキテクチャ方針・ドメイン境界・テスト方針の判断は自分で行い、Cursor はコーディング速度と一貫性の維持に使っています。

---

## ひとことで言うと

**メイン PC をサーバー兼スマートディスプレイにし、スマホ PWA からも同じデータを扱えるローカルファースト個人 OS。**

| レイヤ | 技術 |
|--------|------|
| Desktop shell | Tauri v2 + React 19 |
| HTTP API | Axum 0.8（同一プロセス内） |
| 正本 DB | SQLite（WAL） |
| ドメイン連動 | `tokio::sync::broadcast` EventBus |
| オフライン | Dexie (IndexedDB) + Outbox |
| 端末間同期 | Tailscale P2P → PC Axum + Bearer |
| 鍵配布 | X25519 wrap → PWA WebCrypto（non-extractable） |
| AI / Voice | Gemini ストリーム → 句読点チャンク → ローカル TTS |

---

## 設計原則（憲法）

### 1. Local-first

- **正本は PC 上の SQLite**。スマホは Dexie キャッシュ。
- オフラインでも UI を止めない。失敗した変更は **Outbox** に積み、復帰後に flush。
- `navigator.storage.persist()` でキャッシュ揮発を抑止。

### 2. Event-driven Modular Monolith

- `domains/*` 同士の直接 import は禁止。連動はすべて **EventBus**。
- 例: タスク完了 → 報酬ガチャ → SSE、アラーム停止 → 朝ブリーフィング、買い物下書き → タスク生成。

### 3. 0ms 応答

- 自ドメインの DB 更新が終わった時点で **即 HTTP 200**。
- ガチャ・AI・TTS など他ドメイン処理は EventBus 経由でバックグラウンド。

### 4. Tailscale P2P 同期（多層防御）

1. Tailscale 網内のみ  
2. middleware で loopback + CGNAT (`100.64.0.0/10`) 以外を拒否  
3. リモートは Bearer 必須  
4. 衝突は **SyncRecord.version による LWW**

### 5. ドメイン駆動 AI

- `domains/ai` = Gemini 呼び出し + トークンログのみ。
- プロンプト／パースは呼び出し元ドメインの `ai_handler`。

---

## ランタイム全体像

```
┌───────────────────── メインPC ─────────────────────┐
│  Tauri + React（LayoutMode: Desktop/Resident/…）     │
│       ↕ 同一プロセス                                 │
│  Axum  ←── Tailscale スマホ PWA                      │
│       ↕                                              │
│  SQLite (WAL)  +  EventBus                           │
│       ↕                                              │
│  Gemini / ローカル音声サーバ（WAV 再生キュー）         │
└──────────────────────────────────────────────────────┘
         ▲  Bearer + Tailscale
┌────────┴────────┐
│ Dexie + Outbox  │
│ X25519 鍵受取   │
└─────────────────┘
```

---

## このフォルダの構成

```
public_portfolio/
├── README.md
├── backend-core-samples/
│   ├── core/
│   │   ├── event_bus.rs / event_subscriber_runtime.rs
│   │   ├── error.rs                     # Axum IntoResponse（409 含む）
│   │   ├── crypto_resilient.rs / device_key_wrap.rs / db_wal.rs
│   ├── http/build_router.rs             # CORS + Trace + middleware + bind
│   ├── sync/                            # Tailscale gate / LWW / evaluate
│   └── domains/
│       ├── task/models.rs + services.rs # sqlx CRUD + ユニットテスト
│       ├── sleep/models.rs + services.rs# アラーム CRUD + 午前ガード + テスト
│       ├── reward/gacha_math.rs + gacha_services.rs  # 曲線・予算・sqlx
│       ├── task/ai_handler.rs           # 優先度 AI（EventBus）
│       ├── ai/ · knowledge/ · sleep(stream) · cross_domain/
├── frontend-sync-samples/
│   ├── dexie/offlineDb.ts               # フルスキーマ + Outbox
│   ├── api/offlineFirstApi.ts           # キャッシュ / Outbox / 楽観更新
│   ├── sync/lwwApply.ts                 # 多ドメイン LWW
│   ├── sync/syncPolicy.ts · crypto/ · layout/
├── voice-server-samples/WavPlayQueue.cs
└── tests-samples/                       # 本番 tests/ 相当の厚い契約・境界・MC/DC
    ├── common/test_harness.rs
    ├── lww_conflict_tests.rs
    ├── reward_bus_integration.rs
    └── README.md
```

サーバーPC向け管理ツール（WinForms）は運用寄りのため、本サンプル集には含めていません。

各ファイル先頭に読みどころを短く記しています。機密（実 API キー・個人パス・実 DB）は含めていません。

---

## スタック（抜粋）

**Frontend:** React 19 / TypeScript / Vite / Dexie 4 / WebCrypto / PWA  
**Backend:** Rust / Tauri 2 / Axum 0.8 / Tokio / sqlx + SQLite  
**Sync / Security:** Tailscale · Bearer · X25519 wrap · version LWW  
**AI / Voice:** Gemini · 句読点チャンク · .NET ローカル TTS  
**Tooling:** Cursor（AI 支援 IDE）+ 手動のアーキテクチャ判断

---

## 注意

- 本サンプルは **コンパイル可能な単体クレートではありません**（依存・モジュール境界は本番リポジトリ側）。
- 設計レビュー・技術面談用の「読み物としてのコード」としてご利用ください。
