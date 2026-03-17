# SVG Infographic Design Guide

`insights.svg` を生成する際のデザインガイドラインです。

## Design Philosophy

- **一目で一日がわかる** — 読者が3秒で全体像を把握できること
- **インフォグラフィック風** — アイコン・色彩・レイアウトで情報を視覚的に伝える
- **丁寧で品のあるデザイン** — 不特定多数の読者に適した、落ち着きのある色使い

## Layout Structure

```
┌─────────────────────────────────────────────────────┐
│  📅 Header: Date + Highlight                         │
│  (accent background, large text)                     │
├─────────────────────────────────────────────────────┤
│                                                      │
│  📋 Activities        │  💡 Insights                 │
│  (icon list/flow)     │  (cards with emphasis)       │
│                       │                              │
├───────────────────────┴─────────────────────────────┤
│                                                      │
│  🚧 Challenges        │  🎯 Next Steps              │
│  (status badges)      │  (priority indicators)       │
│                       │                              │
├─────────────────────────────────────────────────────┤
│  🔗 Past Connection (timeline thread, if applicable) │
└─────────────────────────────────────────────────────┘
```

## Color Palette

Warm, professional infographic palette:

| Role | Color | Hex | Usage |
|------|-------|-----|-------|
| Primary | Deep Blue | `#2B4C7E` | Headers, key text |
| Accent | Warm Orange | `#E8834A` | Highlights, emphasis |
| Insight | Golden Yellow | `#F2C94C` | Insights section |
| Challenge | Soft Red | `#EB5757` | Challenges, blockers |
| Success | Teal Green | `#27AE60` | Completed items, next steps |
| Background | Off-White | `#FAFBFC` | Main background |
| Card BG | Light Gray | `#F0F2F5` | Section cards |
| Text | Dark Gray | `#333333` | Body text |
| Subtext | Medium Gray | `#828282` | Secondary info |

## Typography

```svg
<!-- Title -->
<text font-family="system-ui, -apple-system, sans-serif" font-size="28" font-weight="700" fill="#2B4C7E">

<!-- Section Header -->
<text font-family="system-ui, -apple-system, sans-serif" font-size="18" font-weight="600" fill="#2B4C7E">

<!-- Body Text -->
<text font-family="system-ui, -apple-system, sans-serif" font-size="14" font-weight="400" fill="#333333">

<!-- Small / Sub Text -->
<text font-family="system-ui, -apple-system, sans-serif" font-size="12" font-weight="400" fill="#828282">
```

## Icon Approach

Use emoji characters as text elements for icons. They render well in modern SVG viewers:

```svg
<text font-size="24" x="20" y="40">🌟</text>
<text font-size="20" x="20" y="40">📋</text>
<text font-size="20" x="20" y="40">💡</text>
<text font-size="20" x="20" y="40">🚧</text>
<text font-size="20" x="20" y="40">🎯</text>
```

For simple geometric icons, use SVG primitives:

```svg
<!-- Circle indicator -->
<circle cx="10" cy="10" r="6" fill="#27AE60" />

<!-- Priority bar -->
<rect x="0" y="0" width="4" height="24" rx="2" fill="#E8834A" />

<!-- Connecting line -->
<line x1="0" y1="0" x2="100" y2="0" stroke="#DADCE0" stroke-width="1" stroke-dasharray="4,4" />
```

## Section Cards

Each section should be a rounded rectangle card:

```svg
<!-- Section card -->
<rect x="20" y="100" width="360" height="200" rx="12" fill="#F0F2F5" />

<!-- With subtle shadow effect -->
<rect x="20" y="100" width="360" height="200" rx="12" fill="#F0F2F5"
      filter="url(#shadow)" />

<defs>
  <filter id="shadow" x="-2%" y="-2%" width="104%" height="104%">
    <feDropShadow dx="0" dy="2" stdDeviation="4" flood-color="#00000010" />
  </filter>
</defs>
```

## Priority Indicators

```svg
<!-- High priority: filled circle -->
<circle cx="10" cy="10" r="6" fill="#EB5757" />
<text x="22" y="14" font-size="12" fill="#EB5757" font-weight="600">高</text>

<!-- Medium priority -->
<circle cx="10" cy="10" r="6" fill="#E8834A" />
<text x="22" y="14" font-size="12" fill="#E8834A" font-weight="600">中</text>

<!-- Low priority -->
<circle cx="10" cy="10" r="6" fill="#27AE60" />
<text x="22" y="14" font-size="12" fill="#27AE60" font-weight="600">低</text>
```

## Status Badges

```svg
<!-- Status: 未着手 -->
<rect x="0" y="0" width="56" height="22" rx="11" fill="#F0F2F5" />
<text x="28" y="15" text-anchor="middle" font-size="11" fill="#828282">未着手</text>

<!-- Status: 調査中 -->
<rect x="0" y="0" width="56" height="22" rx="11" fill="#FFF3E0" />
<text x="28" y="15" text-anchor="middle" font-size="11" fill="#E8834A">調査中</text>

<!-- Status: 対応中 -->
<rect x="0" y="0" width="56" height="22" rx="11" fill="#E8F5E9" />
<text x="28" y="15" text-anchor="middle" font-size="11" fill="#27AE60">対応中</text>
```

## Responsive Height

SVG height should scale to content. Estimate:
- Header: 80px
- Each section: 150-250px (depends on items)
- Past connection: 80px (if applicable)
- Spacing between sections: 20px
- Padding: 40px top/bottom

Typical total: 600-1200px depending on content volume.

## SVG Skeleton

```svg
<svg xmlns="http://www.w3.org/2000/svg" width="800" height="{calculated}"
     viewBox="0 0 800 {calculated}" font-family="system-ui, -apple-system, sans-serif">

  <defs>
    <filter id="shadow" x="-2%" y="-2%" width="104%" height="104%">
      <feDropShadow dx="0" dy="2" stdDeviation="4" flood-color="#00000010" />
    </filter>
  </defs>

  <!-- Background -->
  <rect width="800" height="{calculated}" fill="#FAFBFC" />

  <!-- Header -->
  <rect x="0" y="0" width="800" height="80" fill="#2B4C7E" />
  <text x="40" y="35" font-size="14" fill="#FFFFFF" opacity="0.8">📅 {yyyy年mm月dd日（曜日）}</text>
  <text x="40" y="60" font-size="22" font-weight="700" fill="#FFFFFF">{highlight}</text>

  <!-- Activities Section (left) -->
  <!-- Insights Section (right) -->
  <!-- Challenges Section (left) -->
  <!-- Next Steps Section (right) -->
  <!-- Past Connection (full width, bottom) -->

</svg>
```

## Best Practices

1. **テキストは短く** — SVG内のテキストは20文字以内を目安に。詳細はdaily.mdに委ねる
2. **色で情報を伝える** — ステータスや優先度は色で直感的に判別可能に
3. **余白を十分に** — 要素間に最低20pxの余白。詰め込みすぎない
4. **階層を明確に** — フォントサイズと太さで情報の優先度を表現
5. **アイコンで視線を誘導** — 各セクションの先頭にアイコンを配置
