# SVG Infographic Design Guide

`insights.svg` を生成する際のデザインガイドラインです。

## Design Philosophy

Google Material Design 3 / Apple HIG + **行動経済学**のベストプラクティスに準拠:

### Core Principles
- **一目で一日がわかる** — 読者が3秒で全体像を把握できること
- **Clarity（明瞭さ）** — 情報階層をタイポグラフィと色で明確に
- **Depth（奥行き）** — Elevation（影）とレイヤーで空間を表現
- **8px Grid** — すべてのサイズ・余白を 8 の倍数に揃える

### Behavioral Economics Principles（注意を惹きつけるデザイン）

| 原則 | 適用箇所 | 実装方法 |
|------|----------|----------|
| **アンカリング効果** | Hero Header | 大きな数字（タスク数・Insight数）を最初に見せ、成果の印象を固定する |
| **Von Restorff効果（孤立効果）** | KEY INSIGHT カード | 他と異なる背景色（ゴールドグラデ）・太いアクセントバー・elevation で際立たせる |
| **損失回避** | Challenges セクション | 赤系の強い配色 + 塗りつぶしバッジ（「未着手」）+ 「要対応」ラベルで行動を促す |
| **ピーク・エンドの法則** | Key Theme / Growth Path | 最も視覚的にリッチなセクション。ダークグラデ + グロー + 大きなフォント |
| **ドーパミン報酬** | Growth Path | プログレスバーのグラデーション + 達成マーカー（塗りつぶし円）で達成感を演出 |
| **認知的流暢性** | 全体 | 高コントラスト（ダーク背景に白文字、白背景に黒文字）で読みやすく |
| **ユーモア・サプライズ** | セクションヘッダー | カウント表示（x5）、遊び心のあるマイクロコピーで記憶に残す |

## Canvas Size

- **幅: 1200px**（横長レイアウト）
- **高さ: 動的計算**（コンテンツ量に応じて算出）

## Layout Structure

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Hero Header (100px fixed, horizontal layout)  ← アンカリング            │
│  [Date chip]  [Title + Subtitle]  [Stats badges]  [Icon]                │
├──────────────────────────────────────────────────────────────────────────┤
│  gap: 24px                                                               │
├───────────────────────────────┬──────────────────────────────────────────┤
│  ACTIVITIES (left, w=564)     │  INSIGHTS (right, w=564)                 │
│  x=24, height: dynamic       │  x=612, ← Von Restorff (KEY INSIGHT)     │
│  (row height = max of both)                                              │
├───────────────────────────────┴──────────────────────────────────────────┤
│  gap: 24px                                                               │
├───────────────────────────────┬──────────────────────────────────────────┤
│  CHALLENGES (left, w=564)     │  NEXT STEPS (right, w=564)               │
│  x=24, ← 損失回避             │  x=612, ← ドーパミン (priority pills)    │
│  (row height = max of both)                                              │
├───────────────────────────────┴──────────────────────────────────────────┤
│  gap: 24px                                                               │
├──────────────────────────────────────────────────────────────────────────┤
│  KEY THEME Banner (x=24, w=1152, h=120)  ← ピーク効果                   │
├──────────────────────────────────────────────────────────────────────────┤
│  gap: 24px                                                               │
├──────────────────────────────────────────────────────────────────────────┤
│  GROWTH / WORKFLOW Section (x=24, w=1152, h=160-192)  ← エンド効果       │
├──────────────────────────────────────────────────────────────────────────┤
│  Footer (40px)                                                           │
└──────────────────────────────────────────────────────────────────────────┘
```

## Dynamic Height Calculation (CRITICAL)

カードの高さは**コンテンツ量に応じて動的に算出**すること。固定値でのハードコーディングは禁止。

### Layout Constants

| Constant | Value | Description |
|----------|-------|-------------|
| `SVG_WIDTH` | 1200px | キャンバス幅 |
| `HERO_HEIGHT` | 100px | ヘッダー高さ |
| `LEFT_CARD_X` | 24px | 左カード X位置 |
| `RIGHT_CARD_X` | 612px | 右カード X位置 |
| `CARD_WIDTH` | 564px | 各カード幅 |
| `LEFT_ITEM_WIDTH` | 520px | 左カード内アイテム幅 |
| `RIGHT_ITEM_WIDTH` | 516px | 右カード内アイテム幅 |
| `FULL_WIDTH_X` | 24px | 全幅セクション X |
| `FULL_WIDTH_W` | 1152px | 全幅セクション幅 |
| `CARD_HEADER` | 76px | カード上部余白（bar + label + title） |
| `CARD_PAD_BOTTOM` | 20px | カード下部余白 |
| `SECTION_GAP` | 24px | セクション間の余白 |

### Card Label / Title Position（カード y からの相対オフセット）

| 要素 | オフセット | 説明 |
|------|-----------|------|
| Section label | y + 36 | ACTIVITIES, INSIGHTS 等（11px, uppercase） |
| Section title | y + 58 | 実施事項, 気づき・学び 等（18px） |
| Humor count | y + 58 | x5 等（右端 text-anchor="end"） |
| Content start | y + 76 | アイテム群の translate Y |

### Item Heights

| Section | Item Type | Item Height | Item Gap |
|---------|-----------|-------------|----------|
| Activities | list row | 40px | 8px |
| Insights | detail card | 88px | 12px |
| Challenges | status row | 44px | 10px |
| Next Steps | priority row | 36px | 8px |

### Card Height Formula

```
card_height = CARD_HEADER + (item_count × item_height) + ((item_count - 1) × item_gap) + CARD_PAD_BOTTOM
```

### Row Height (2-column layout)

```
row_height = max(left_card_height, right_card_height)
```

**同一行の両カードには必ず同じ `height` を設定すること。**

### Total SVG Height

```
total = HERO_HEIGHT(100) + gap(24)
      + row1_height + gap(24)
      + row2_height + gap(24)
      + key_theme(120) + gap(24)
      + growth_or_workflow(160-192, optional) + gap(24, if exists)
      + footer(40)
```

### Y Position Formula

```
row1_y     = HERO_HEIGHT + SECTION_GAP           = 124
row2_y     = row1_y + row1_height + SECTION_GAP
keytheme_y = row2_y + row2_height + SECTION_GAP
growth_y   = keytheme_y + 120 + SECTION_GAP
footer_y   = growth_y + growth_height + SECTION_GAP + 16
```

### Example: 5 activities, 4 insights, 2 challenges, 3 next steps

```
activities_h = 76 + (5×40) + (4×8) + 20 = 328
insights_h   = 76 + (4×88) + (3×12) + 20 = 484
row1 = max(328, 484) = 484

challenges_h = 76 + (2×44) + (1×10) + 20 = 194
nextsteps_h  = 76 + (3×36) + (2×8)  + 20 = 220
row2 = max(194, 220) = 220

row1_y = 124
row2_y = 124 + 484 + 24 = 632
keytheme_y = 632 + 220 + 24 = 876
growth_y = 876 + 120 + 24 = 1020
footer_y = 1020 + 192 + 24 + 16 = 1252
total = 100 + 24 + 484 + 24 + 220 + 24 + 120 + 24 + 192 + 24 + 40 = 1276
```

## Color Palette（行動経済学ベース）

高彩度・高コントラストで注意を引きつける配色。

### Primary Gradients

| Name | From → To | 心理効果 | Usage |
|------|-----------|----------|-------|
| `heroGrad` | `#0a0a1a` → `#1a237e` | 権威・信頼 | Hero header |
| `peakGrad` | `#1a237e` → `#4A148C` | 高揚・プレミアム感 | Key Theme banner |
| `accentGrad` | `#536DFE` → `#7C4DFF` | 集中・知性 | Activities accent |
| `warmGrad` | `#FF6D00` → `#FF1744` | 緊張・注目 | KEY INSIGHT accent |
| `sunGrad` | `#FF6D00` → `#FFAB00` | エネルギー・楽観 | Insights section bar |
| `tealGrad` | `#00B0FF` → `#00E5FF` | 安心・開放感 | Stats badge |
| `greenGrad` | `#00C853` → `#69F0AE` | 成長・報酬 | Next Steps accent |
| `dangerGrad` | `#FF1744` → `#FF5252` | 危機感・損失回避 | Challenges accent |
| `progressGrad` | `#536DFE` → `#00E5FF` → `#69F0AE` | 進展・ドーパミン | Progress bar |
| `keyInsightBg` | `#FFF3E0` → `#FFE0B2` | 温かみ・特別感 | KEY INSIGHT bg |

### Flat Colors

| Role | Hex | Usage |
|------|-----|-------|
| Background | `#f5f5f7` | Main canvas |
| Card | `#FFFFFF` | Card surface |
| Text Primary | `#0a0a1a` | Headings |
| Text Body | `#5D4037` | Description |
| Text Tertiary | `#78909C` | Labels |
| Text Footer | `#B0BEC5` | Footer |
| Challenge Red | `#FF1744` | Badge, label |
| Challenge Text | `#B71C1C` | Item text |
| Challenge BG | `#FFEBEE` | Item bg |
| Insight Orange | `#E65100` | Insight title |
| Insight BG | `#FFF3E0` | Insight card bg |
| Activity BG | `#EDE7F6` | Activity item bg |
| Next Step P1 BG | `#FFEBEE` | P1 bg |
| Next Step P2 BG | `#FFF3E0` | P2 bg |
| Next Step P3 BG | `#E8F5E9` | P3 bg |

## Typography

```svg
font-family="'SF Pro Display', 'Inter', system-ui, -apple-system, 'Segoe UI', sans-serif"

<!-- Hero Title: 22px/800, letter-spacing: -0.5 -->
<!-- Key Theme Title: 22px/800, letter-spacing: -0.5 -->
<!-- Section Title: 18px/800, letter-spacing: -0.3 -->
<!-- Section Label: 11px/800, letter-spacing: 1.5, uppercase -->
<!-- Body: 14px/600 -->
<!-- Description: 12px/400-500 -->
<!-- Small/Label: 11-12px/600-700 -->
<!-- Footer: 11px/400, letter-spacing: 0.5 -->
```

font-weight は **600-800** を使用。

## Elevation

```svg
<!-- Level 1: Standard cards -->
<filter id="elevation1" x="-4%" y="-4%" width="108%" height="112%">
  <feDropShadow dx="0" dy="1" stdDeviation="3" flood-color="rgba(0,0,0,0.07)" />
  <feDropShadow dx="0" dy="4" stdDeviation="8" flood-color="rgba(0,0,0,0.05)" />
</filter>

<!-- Level 2: Peak cards (Key Theme, Growth) -->
<filter id="elevation2" x="-4%" y="-4%" width="108%" height="116%">
  <feDropShadow dx="0" dy="2" stdDeviation="8" flood-color="rgba(0,0,0,0.10)" />
  <feDropShadow dx="0" dy="10" stdDeviation="20" flood-color="rgba(0,0,0,0.08)" />
</filter>
```

## Card Structure

```svg
<!-- Card container (left example) -->
<rect x="24" y="{card_y}" width="564" height="{row_height}" rx="16" fill="#FFFFFF" filter="url(#elevation1)" />
<rect x="24" y="{card_y}" width="564" height="4" rx="2" fill="url(#{gradient})" />
<text x="48" y="{card_y+36}" font-size="11" font-weight="800" fill="{accent}" letter-spacing="1.5">{LABEL}</text>
<text x="48" y="{card_y+58}" font-size="18" font-weight="800" fill="#0a0a1a" letter-spacing="-0.3">{タイトル}</text>
<text x="550" y="{card_y+58}" text-anchor="end" font-size="12" font-weight="600" fill="{accent}">x{count}</text>

<!-- Card container (right example) -->
<rect x="612" y="{card_y}" width="564" height="{row_height}" rx="16" fill="#FFFFFF" filter="url(#elevation1)" />
<rect x="612" y="{card_y}" width="564" height="4" rx="2" fill="url(#{gradient})" />
<text x="636" y="{card_y+36}" font-size="11" font-weight="800" fill="{accent}" letter-spacing="1.5">{LABEL}</text>
<text x="636" y="{card_y+58}" font-size="18" font-weight="800" fill="#0a0a1a" letter-spacing="-0.3">{タイトル}</text>

<!-- Content area: translate(content_x, card_y + 76) -->
<g transform="translate(48, {card_y+76})">  <!-- left card content -->
<g transform="translate(636, {card_y+76})"> <!-- right card content -->
```

## Item Templates

### Activity Item
```svg
<rect x="0" y="{i*(40+8)}" width="520" height="40" rx="10" fill="#EDE7F6" />
<rect x="12" y="{i*(40+8)+12}" width="16" height="16" rx="4" fill="url(#accentGrad)" />
<text x="38" y="{i*(40+8)+26}" font-size="14" font-weight="600" fill="#0a0a1a">{text}</text>
```

### Insight Card（通常）
```svg
<g transform="translate(0, {i*(88+12)})">
  <rect x="0" y="0" width="516" height="88" rx="12" fill="#FFF3E0" />
  <rect x="0" y="0" width="4" height="88" rx="2" fill="url(#sunGrad)" />
  <text x="16" y="24" font-size="14" font-weight="700" fill="#E65100">{title}</text>
  <text x="16" y="44" font-size="12" fill="#5D4037">{line1}</text>
  <text x="16" y="62" font-size="12" fill="#5D4037">{line2}</text>
  <rect x="16" y="72" width="{tag_w}" height="12" rx="6" fill="#FF6D00" />
  <text x="{tag_center}" y="82" text-anchor="middle" font-size="8" font-weight="700" fill="#FFFFFF">{TAG}</text>
</g>
```

### Insight Card（KEY INSIGHT — Von Restorff効果）
```svg
<g transform="translate(0, {i*(88+12)})">
  <rect x="0" y="0" width="516" height="88" rx="12" fill="url(#keyInsightBg)" filter="url(#elevation1)" />
  <rect x="0" y="0" width="6" height="88" rx="3" fill="url(#warmGrad)" />
  <text x="16" y="24" font-size="14" font-weight="800" fill="#BF360C">{title}</text>
  <text x="16" y="44" font-size="12" font-weight="500" fill="#4E342E">{line1}</text>
  <text x="16" y="62" font-size="12" font-weight="500" fill="#4E342E">{line2}</text>
  <rect x="16" y="72" width="80" height="12" rx="6" fill="url(#warmGrad)" />
  <text x="56" y="82" text-anchor="middle" font-size="8" font-weight="800" fill="#FFFFFF">KEY INSIGHT</text>
</g>
```

### Challenge Item（損失回避）
```svg
<g transform="translate(0, {i*(44+10)})">
  <rect x="0" y="0" width="520" height="44" rx="10" fill="#FFEBEE" />
  <rect x="0" y="0" width="4" height="44" rx="2" fill="#FF1744" />
  <text x="16" y="27" font-size="14" font-weight="600" fill="#B71C1C">{text}</text>
  <rect x="420" y="12" width="80" height="22" rx="11" fill="#FF1744" />
  <text x="460" y="27" text-anchor="middle" font-size="10" font-weight="700" fill="#FFFFFF">{status}</text>
</g>
```

### Next Step Item
```svg
<g transform="translate(0, {i*(36+8)})">
  <rect x="0" y="0" width="516" height="36" rx="8" fill="{priority_bg}" />
  <rect x="8" y="8" width="10" height="20" rx="5" fill="{priority_color}" />
  <text x="26" y="14" font-size="11" font-weight="800" fill="{priority_color}">P{n}</text>
  <text x="52" y="24" font-size="14" font-weight="600" fill="#0a0a1a">{text}</text>
</g>
```

## Priority / Status

```svg
<!-- P1 (High) → #FF1744, bg: #FFEBEE -->
<!-- P2 (Medium) → #FF6D00, bg: #FFF3E0 -->
<!-- P3 (Low) → #00C853, bg: #E8F5E9 -->

<!-- 未着手: fill="#FF1744" + text="#FFFFFF" -->
<!-- 調査中: fill="#FF6D00" + text="#FFFFFF" -->
<!-- 対応中: fill="#536DFE" + text="#FFFFFF" -->
<!-- 完了:   fill="#00C853" + text="#FFFFFF" -->
```

## Hero Header（100px, 横展開）

```svg
<rect width="1200" height="100" fill="url(#heroGrad)" clip-path="url(#heroClip)" />

<!-- Decorative circles -->
<circle cx="1100" cy="20" r="120" fill="#536DFE" opacity="0.07" />
<circle cx="1140" cy="80" r="70" fill="#7C4DFF" opacity="0.05" />
<circle cx="50" cy="80" r="40" fill="#00E5FF" opacity="0.04" />

<!-- Icon (circular, top-right) -->
<g transform="translate(1116, 10)">
  <circle cx="28" cy="28" r="30" fill="rgba(255,255,255,0.15)" />
  <image href="data:image/jpeg;base64,{b64}" x="-12" y="-12" width="80" height="80"
         preserveAspectRatio="xMidYMid meet" clip-path="url(#iconClip)" />
</g>

<!-- Date chip -->
<rect x="40" y="12" width="200" height="24" rx="12" fill="rgba(255,255,255,0.1)" />
<text x="56" y="29" font-size="12" font-weight="500" fill="rgba(255,255,255,0.8)">{date}</text>

<!-- Title + subtitle -->
<text x="40" y="52" font-size="22" font-weight="800" fill="#FFFFFF" letter-spacing="-0.5">{title}</text>
<text x="40" y="74" font-size="13" font-weight="400" fill="rgba(255,255,255,0.6)">{subtitle}</text>

<!-- Stats badges -->
<g transform="translate(860, 68)">
  <rect width="84" height="22" rx="11" fill="url(#accentGrad)" opacity="0.9" />
  <text x="12" y="16" font-size="13" font-weight="800" fill="#FFFFFF">{n}</text>
  <text x="28" y="15" font-size="10" font-weight="500" fill="rgba(255,255,255,0.85)">TASKS</text>
  <rect x="92" width="108" height="22" rx="11" fill="url(#sunGrad)" opacity="0.9" />
  <text x="104" y="16" font-size="13" font-weight="800" fill="#FFFFFF">{n}</text>
  <text x="120" y="15" font-size="10" font-weight="500" fill="rgba(255,255,255,0.9)">INSIGHTS</text>
</g>
```

### アイコン埋め込み手順

1. `assets/icons/` 配下の画像ファイルを使用
2. `base64 -w 0 {image_path}` でエンコード
3. `data:image/jpeg;base64,{encoded}` 形式で `<image href="...">` に埋め込む
4. `preserveAspectRatio="xMidYMid meet"` で画像全体が見えるようにフィット
5. `clipPath="url(#iconClip)"` で円形にクリップ（r=28）
6. 白い半透明リング（r=30）をバックに配置

### defs に必要なクリップパス

```svg
<clipPath id="iconClip">
  <circle cx="28" cy="28" r="28" />
</clipPath>
<clipPath id="heroClip">
  <rect width="1200" height="100" />
</clipPath>
```

## Key Theme Banner（120px, ピーク効果）

```svg
<rect x="24" y="{y}" width="1152" height="120" rx="20" fill="url(#peakGrad)" filter="url(#elevation2)" />
<circle cx="1100" cy="{y+36}" r="60" fill="#7C4DFF" opacity="0.12" />
<circle cx="1060" cy="{y+76}" r="40" fill="#536DFE" opacity="0.08" />
<text x="56" y="{y+34}" font-size="11" font-weight="800" fill="rgba(255,255,255,0.5)" letter-spacing="2.5">TODAY'S KEY THEME</text>
<text x="56" y="{y+64}" font-size="22" font-weight="800" fill="#FFFFFF" letter-spacing="-0.5">{theme}</text>
<text x="56" y="{y+92}" font-size="13" font-weight="400" fill="rgba(255,255,255,0.6)">{description}</text>
```

## Growth Path Section（192px）

```svg
<rect x="24" y="{y}" width="1152" height="192" rx="20" fill="#FFFFFF" filter="url(#elevation2)" />
<text x="56" y="{y+36}" font-size="11" font-weight="800" fill="#536DFE" letter-spacing="1.5">GROWTH PATH</text>
<text x="56" y="{y+58}" font-size="18" font-weight="800" fill="#0a0a1a" letter-spacing="-0.3">{title}</text>

<!-- Progress bar at y+76 -->
<g transform="translate(56, {y+76})">
  <rect x="0" y="16" width="1100" height="10" rx="5" fill="#E8EAF6" />
  <rect x="0" y="16" width="{fill}" height="10" rx="5" fill="url(#progressGrad)" />
  <!-- Achieved marker (solid) -->
  <circle cx="{pos1}" cy="21" r="16" fill="url(#accentGrad)" />
  <!-- Target marker (dashed) -->
  <circle cx="{pos2}" cy="21" r="16" fill="#FFFFFF" stroke="url(#greenGrad)" stroke-width="3" stroke-dasharray="5,3" />
</g>

<!-- Lever tags at y+168 -->
<g transform="translate(56, {y+168})">
  <text x="0" y="14" font-size="12" font-weight="600" fill="#78909C">{label}</text>
  <rect x="{x}" y="0" width="{w}" height="26" rx="13" fill="url(#{grad})" />
  <text x="{center}" y="18" text-anchor="middle" font-size="12" font-weight="700" fill="#FFFFFF">{tag}</text>
</g>
```

## Workflow Section（160px, 代替）

Growth Path の代わりに使用可能:

```svg
<rect x="24" y="{y}" width="1152" height="160" rx="20" fill="url(#peakGrad)" filter="url(#elevation2)" />
<text x="56" y="{y+34}" font-size="11" font-weight="800" fill="rgba(255,255,255,0.5)" letter-spacing="2.5">WORKFLOW OF THE DAY</text>
<text x="56" y="{y+64}" font-size="20" font-weight="800" fill="#FFFFFF">{title}</text>

<!-- Flow steps at y+80, evenly distributed across 1100px -->
<g transform="translate(56, {y+80})">
  <rect x="0" y="0" width="140" height="32" rx="8" fill="url(#accentGrad)" />
  <text x="70" y="21" text-anchor="middle" font-size="12" font-weight="700" fill="#FFFFFF">{step1}</text>
  <text x="156" y="21" font-size="16" fill="rgba(255,255,255,0.4)">→</text>
  <!-- repeat for each step -->
</g>

<text x="56" y="{y+140}" font-size="12" fill="rgba(255,255,255,0.5)">{description}</text>
```

## Footer

```svg
<text x="600" y="{footer_y}" text-anchor="middle" font-size="11" font-weight="400" fill="#B0BEC5" letter-spacing="0.5">LifeAI Daily Insights — {date}</text>
```

## Best Practices

1. **カード高さは必ず動的計算** — 項目数に応じて算出し、はみ出しを防ぐ
2. **同一行のカードは高さを揃える** — `max()` で統一。y 位置も完全一致させる
3. **Y Position Formula に従う** — card_y + 36 = label, + 58 = title, + 76 = content
4. **8px grid を遵守** — すべての値を 8 の倍数に（例外: 4px accent bar）
5. **Von Restorff** — KEY INSIGHT は必ず他と異なるスタイルで際立たせる
6. **損失回避** — Challenges は塗りつぶし赤バッジ + 左ボーダー + 「要対応」ラベル
7. **アンカリング** — Hero に大きな数字を配置
8. **ピーク・エンド** — Key Theme と Growth は最もビジュアルリッチに
9. **font-weight 600-800** — 太字は記憶に残る。500以下は description のみ
10. **高コントラスト** — ダーク背景に白、白背景に #0a0a1a
11. **SVG height と viewBox を一致させる** — 算出した total を両方に設定
12. **left x=24/48, right x=612/636** — カード/コンテンツの X 位置を間違えない
