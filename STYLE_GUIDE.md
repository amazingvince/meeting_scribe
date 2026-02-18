# Meeting Scribe — UI Style Guide

This document defines the visual language, component patterns, and conventions for Meeting Scribe's frontend. Follow these rules when building new features or modifying existing UI.

---

## 1. Design Token System

All colors come from CSS custom properties defined in `src/index.css` and registered in `tailwind.config.js`. **Never use raw Tailwind palette colors** (e.g. `bg-indigo-500`, `text-amber-600`) in feature code. Use semantic tokens instead.

### Core Surface Tokens

| Token | Light | Dark | Purpose |
|---|---|---|---|
| `background` | `244 247 251` | `8 11 17` | App background |
| `foreground` | `15 23 42` | `226 232 240` | Primary text |
| `card` / `card-foreground` | white / slate-900 | slate-900 / slate-200 | Card surfaces |
| `primary` / `primary-foreground` | near-black / white | white / near-black | Buttons, controls, progress bars |
| `secondary` / `secondary-foreground` | slate-100 / slate-900 | slate-800 / slate-200 | Secondary buttons, tags |
| `muted` / `muted-foreground` | gray-200 / gray-500 | slate-800 / slate-400 | Disabled states, metadata text |
| `accent` / `accent-foreground` | gray-200 / slate-900 | slate-800 / slate-200 | Hover backgrounds, active nav |
| `destructive` / `destructive-foreground` | red / white | red / white | Delete, errors |
| `border` | `rgba(0,0,0,0.1)` | `rgba(255,255,255,0.1)` | All borders |
| `input` / `input-background` | gray border / gray-100 | white border / slate-800 | Form controls |

### Semantic Color Tokens

These replace all previously scattered palette colors:

| Token | Light | Dark | Use for |
|---|---|---|---|
| `brand` | `99 91 255` (violet) | `129 120 255` | AI features, chat accents, section icons, filter badges, links |
| `highlight` | `245 200 66` (gold) | `250 210 80` | Search result highlighting, word hover in transcripts |
| `success` | `34 197 94` (green) | `74 222 128` | "Ready" states, saved confirmations, active indicators |
| `warning` | `234 179 8` (amber) | `250 204 21` | Unsaved changes, model-not-loaded, needs-attention |
| `info` | `59 130 246` (blue) | `96 165 250` | Informational badges, general status |

### Usage Patterns

```tsx
// ✅ GOOD — semantic tokens
<Sparkles className="w-5 h-5 text-brand" />
<mark className="bg-highlight/60 px-0.5">matched text</mark>
<p className="text-success">All changes saved</p>
<p className="text-warning">Unsaved changes</p>
<div className="bg-brand/5 border-brand/20">processing banner</div>

// ❌ BAD — raw palette colors
<Sparkles className="w-5 h-5 text-indigo-500" />
<mark className="bg-amber-200/80">matched text</mark>
<p className="text-green-600 dark:text-green-400">saved</p>
```

### Alpha Modifiers

Use Tailwind alpha modifiers with semantic tokens for tinted backgrounds and borders:

| Pattern | When to use |
|---|---|
| `bg-brand/5` | Very subtle tinted background (banners) |
| `bg-brand/10` | Light tinted background (avatar containers, badges) |
| `bg-brand/20` | Medium tint (hover states on brand surfaces) |
| `border-brand/20` | Tinted borders (processing banners, selected items) |
| `bg-highlight/40` | Transcript word hover |
| `bg-highlight/60` | Search result highlights |
| `text-brand/70` | De-emphasized brand text |

### Exceptions: When Raw Palette Colors Are OK

Raw Tailwind palette colors are acceptable **only** in base UI primitives that define the design system itself:

- `Badge.tsx` — variant definitions (`bg-green-100`, `bg-red-100`, etc.)
- `Toast.tsx` — toast type styles
- `StatusBadge` — status dot colors (`bg-red-500`, `bg-green-500`, `bg-yellow-500`)
- `Layout.tsx` — recording indicator (`bg-red-500`, animation)
- `RecordingView.tsx` — recording dot indicator, error banners

The rule of thumb: if it's a **reusable primitive** defining the system, palette colors are fine. If it's a **feature component**, use tokens.

---

## 2. Typography

### Global Heading Normalization

Defined in `@layer base` in `index.css`:

```css
h2 { font-size: 1rem; font-weight: 600; line-height: 1.4; }
h3 { font-size: 0.875rem; font-weight: 600; line-height: 1.4; }
```

This means `<h2>` and `<h3>` elements have consistent sizing app-wide without needing utility classes for size/weight. Override with Tailwind only when you need to deviate (e.g. modal titles use `text-lg`).

### Type Scale in Use

| Element | Classes | When |
|---|---|---|
| Page/section header | `<h2>` (inherits base) or `text-foreground` | Section titles in headers |
| Card title | `text-base font-semibold` (via `CardTitle`) | Card headings |
| Body text | `text-sm text-foreground/90` | Transcript text, chat messages |
| Metadata | `text-xs text-muted-foreground` | Timestamps, file info, hints |
| Micro-labels | `text-[11px] text-muted-foreground` | "Sources:", track labels, segment counts |
| Uppercase labels | `text-[11px] font-semibold uppercase tracking-wide text-muted-foreground` | Filter labels, section dividers |
| Monospace | `font-mono text-xs` or `tabular-nums font-mono` | Timestamps, durations |

### Font

Inter is the primary font, loaded via Google Fonts with weights 300/400/500/600. The `font-sans` Tailwind token maps to `['Inter', 'Segoe UI', 'system-ui', 'sans-serif']`.

---

## 3. Spacing & Layout

### Border Radius

`--radius: 0.75rem` (12px). Tailwind's `rounded-lg` maps to this. Common patterns:

| Class | Value | Use for |
|---|---|---|
| `rounded-xl` | 16px | Cards, modals, nav pills, tabs container |
| `rounded-lg` | 12px (`--radius`) | Inner containers, inputs, buttons |
| `rounded-md` | 10px | Small controls, select elements |
| `rounded-full` | pill | Badges, avatars, progress bars, toggle switches |
| `rounded-2xl` | 20px | Large icon containers (empty states, chat hero) |

### Page-Level Layout

Each view owns its own padding and scrolling. The `<main>` wrapper provides zero padding.

```
┌─ sidebar (72px, hidden <md) ─┐ ┌─ main content ─────────────────┐
│  logo + nav                  │ │  <header> px-6 py-4 border-b   │
│                              │ │  <content> owns its own scroll  │
│                              │ │  <footer/input> if applicable   │
└──────────────────────────────┘ └─────────────────────────────────┘
                                 ┌─ mobile nav (fixed bottom, <md) ─┐
```

Standard page header pattern:
```tsx
<header className="flex items-center justify-between px-6 py-4 border-b border-border bg-card">
```

Standard content area pattern:
```tsx
<div className="no-scrollbar flex-1 min-h-0 overflow-y-auto p-4 md:p-5 space-y-5">
```

### Card Spacing

Cards use `p-4` (md) by default. Nested content uses `space-y-*` for vertical rhythm:
- `space-y-2` — tight (form fields, metadata lines)
- `space-y-3` — standard (audio player sections)
- `space-y-4` — medium (card sections)
- `space-y-5` — roomy (top-level panel sections)
- `space-y-6` — spacious (settings page cards)

---

## 4. Component Library (`src/components/ui/`)

All primitives live in `src/components/ui/` and are re-exported from `index.ts`. Always import from the specific file or the barrel export.

### Button

```tsx
import { Button } from '../ui/Button';

// Variants: default | secondary | destructive | ghost | outline | link
// Sizes: sm (h-8) | md (h-10) | lg (h-11) | icon (h-10 w-10)
<Button variant="secondary" size="sm" isLoading={loading}>
  <Icon className="w-4 h-4" />
  Label
</Button>
```

Uses `class-variance-authority` (CVA). Has built-in `isLoading` state with spinner. All buttons get `active:scale-[0.985]` micro-interaction.

### Card

```tsx
import { Card, CardTitle } from '../ui/Card';

// padding: none | sm (p-3) | md (p-4, default) | lg (p-6)
// hover: false (default) | true (adds lift + shadow on hover)
<Card hover onClick={...}>
  <CardTitle>Section Title</CardTitle>
  ...
</Card>
```

### Badge

```tsx
import { Badge, StatusBadge } from '../ui/Badge';

// Variants: default | secondary | outline | success | warning | error | info
// Sizes: sm | md
<Badge variant="warning">medium</Badge>
<StatusBadge status="ready" /> // pre-composed with dot indicator
```

Use `<Badge>` for action item priorities, status labels, and filter counts. Use `<StatusBadge>` specifically for meeting status.

### Select

```tsx
import { Select } from '../ui/Select';

<Select id="backend" label="Capture Backend" value={value} onChange={handler}>
  <option value="auto">Auto</option>
  <option value="manual">Manual</option>
</Select>
```

Styled `<select>` with label, error support, focus ring, and disabled state. Use this instead of raw `<select>` elements.

### Toggle

```tsx
import { Toggle } from '../ui/Toggle';

<Toggle
  checked={enabled}
  onChange={setEnabled}
  label="Live Transcript Preview"
  description="Shows rolling transcript snippets while recording."
/>
```

Accessible toggle switch (`role="switch"`, `aria-checked`). Built-in label and description layout.

### Input / Textarea

```tsx
import { Input, Textarea } from '../ui/Input';

<Input label="Name" error={errors.name} leftIcon={<Search />} />
<Textarea label="Notes" hint="Markdown supported" />
```

### Progress

```tsx
import { ProgressBar, Spinner } from '../ui/Progress';

// size: sm (h-1) | md (h-2) | lg (h-3)
<ProgressBar value={65} showLabel label="Transcribing..." />
<Spinner size="lg" />
```

All progress bars use `bg-primary` for the fill. No color prop — the single fill color keeps the UI consistent.

### Modal

```tsx
import { Modal, ModalFooter } from '../ui/Modal';

<Modal isOpen={show} onClose={() => setShow(false)} title="Confirm" size="sm">
  <p>Are you sure?</p>
  <ModalFooter>
    <Button variant="secondary" onClick={close}>Cancel</Button>
    <Button onClick={confirm}>Confirm</Button>
  </ModalFooter>
</Modal>
```

Supports Escape key dismiss, backdrop click, and animated enter/exit.

### Tabs

```tsx
import { Tabs, TabsList, TabsTrigger, TabsContent } from '../ui/Tabs';

<Tabs defaultValue="transcript" value={tab} onValueChange={setTab}>
  <TabsList>
    <TabsTrigger value="transcript">Transcript</TabsTrigger>
    <TabsTrigger value="summary">Summary</TabsTrigger>
  </TabsList>
  <TabsContent value="transcript">...</TabsContent>
  <TabsContent value="summary">...</TabsContent>
</Tabs>
```

Has spring-animated active pill indicator (`layoutId`).

### EmptyState

```tsx
import { EmptyState, NoMeetingsEmpty } from '../ui/EmptyState';

<EmptyState
  icon={<FileQuestion className="w-12 h-12" />}
  title="No transcript"
  description="Record a meeting first."
  action={{ label: 'Record', onClick: goToRecord }}
/>
```

Icons are wrapped in a tinted `bg-muted` container (`h-16 w-16 rounded-2xl`) automatically.

### Toast

Positioned at `fixed top-4 right-4 z-50`. Use the `useToastStore` hook:

```tsx
const toast = useToastStore();
toast.success('Saved');
toast.error('Failed', 'Details here');
toast.warning('Warning title', 'Explanation');
toast.info('Info title');
```

### BackgroundTaskPill

Positioned at `fixed bottom-20 right-4 md:bottom-4 z-40` to avoid overlapping with toasts (top-right) and mobile nav (bottom). Clickable, navigates to the relevant view.

---

## 5. Icons

All icons come from **Lucide React**. Standard sizing:

| Context | Size | Example |
|---|---|---|
| Inline with text | `w-3.5 h-3.5` | Section header labels |
| Button companion | `w-4 h-4` | Button icons, action icons |
| Section header | `w-5 h-5` | Card title icons, nav icons |
| Hero / empty state | `w-7 h-7` or `w-12 h-12` | Chat hero, empty state centers |

### Icon Color Convention

| Meaning | Class |
|---|---|
| AI/brand feature | `text-brand` |
| Success/active | `text-success` |
| Warning/attention | `text-warning` |
| Neutral metadata | `text-muted-foreground` |
| Interactive/primary | `text-primary` |
| Destructive | `text-destructive` |

---

## 6. Animation

Animations use **framer-motion** for layout transitions, and Tailwind for simple state changes.

### Standard Motion Values

```tsx
// Card/item enter
initial={{ opacity: 0, y: 6 }}
animate={{ opacity: 1, y: 0 }}
transition={{ duration: 0.1, ease: [0.22, 1, 0.36, 1] }}

// Modal/toast enter
initial={{ opacity: 0, y: 10, scale: 0.985 }}
animate={{ opacity: 1, y: 0, scale: 1 }}

// Spring for nav pills and tab indicators
transition={{ type: 'spring', stiffness: 460, damping: 36, mass: 0.65 }}
```

### CSS Transitions

- `transition-colors duration-150` — hover/focus color changes
- `transition-all duration-150` — button press, card hover
- `transition-all duration-300` — progress bar fills
- `transition-transform` — toggle knob slide

### Micro-Interactions

- Buttons: `active:scale-[0.985]`
- Hover cards: `hover:-translate-y-0.5 hover:shadow-md` (via `Card hover` prop)
- Meeting cards: `whileHover={{ y: -0.5 }}` (framer-motion)

---

## 7. Dark Mode

Dark mode uses `class` strategy (`darkMode: 'class'` in Tailwind config). The `.dark` class is toggled on the root element.

### Rules

1. **Always use token classes** (`text-foreground`, `bg-card`, `border-border`) — they auto-adapt.
2. **Never use `dark:` prefix** in feature components — semantic tokens handle this.
3. The `dark:` prefix is only acceptable in base primitives (Badge, Toast, StatusBadge) where explicit light/dark variants are defined.
4. Alpha modifiers on tokens work in both modes: `bg-brand/10` auto-adjusts because the `--brand` variable changes.

---

## 8. Markdown Rendering

LLM-generated content (chat responses, meeting summaries) is rendered with `react-markdown` + `remark-gfm`.

### Standard Configuration

```tsx
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

<ReactMarkdown
  remarkPlugins={[remarkGfm]}
  components={{
    p: ({ children }) => <p className="mb-2 last:mb-0">{children}</p>,
    h1: ({ children }) => <h1 className="text-base font-semibold mt-4 mb-2">{children}</h1>,
    h2: ({ children }) => <h2 className="text-sm font-semibold mt-3 mb-1.5">{children}</h2>,
    h3: ({ children }) => <h3 className="text-sm font-semibold mt-2 mb-1">{children}</h3>,
    ul: ({ children }) => <ul className="list-disc pl-5 mb-2 space-y-0.5">{children}</ul>,
    ol: ({ children }) => <ol className="list-decimal pl-5 mb-2 space-y-0.5">{children}</ol>,
    li: ({ children }) => <li>{children}</li>,
    strong: ({ children }) => <strong className="font-semibold">{children}</strong>,
    blockquote: ({ children }) => (
      <blockquote className="border-l-2 border-brand pl-3 my-2 text-muted-foreground italic">
        {children}
      </blockquote>
    ),
    hr: () => <hr className="my-3 border-border" />,
    // For chat: also include code block handling
    // For summaries: simpler subset is fine
  }}
>
  {content}
</ReactMarkdown>
```

### Code Blocks (Chat Only)

```tsx
code: ({ className, children, ...props }) => {
  const isBlock = className?.includes('language-');
  if (isBlock) {
    return (
      <pre className="my-2 rounded-lg bg-muted p-3 overflow-x-auto text-xs">
        <code className={className} {...props}>{children}</code>
      </pre>
    );
  }
  return (
    <code className="rounded bg-muted px-1.5 py-0.5 text-xs font-mono" {...props}>
      {children}
    </code>
  );
},
pre: ({ children }) => <>{children}</>,
```

### Links (Chat Only)

```tsx
a: ({ href, children }) => (
  <a href={href} target="_blank" rel="noopener noreferrer"
     className="text-brand underline underline-offset-2 hover:text-brand/80">
    {children}
  </a>
),
```

---

## 9. Form Patterns

### Labels

Labels are `text-sm font-medium text-foreground` with `mb-2 block`.

### Hint/Help Text

Below form controls: `text-xs text-muted-foreground mt-2`.

### Error Text

Below form controls: `text-xs text-destructive mt-1`.

### Disabled States

All form controls use `disabled:cursor-not-allowed disabled:opacity-50`.

### Form Layout

Stack form fields with `space-y-4`. Group related fields inside a bordered container:
```tsx
<div className="space-y-3 rounded-lg border border-border p-3">
```

---

## 10. Shared Utilities

### `src/utils/stages.ts`

Single source of truth for processing stage labels:

```tsx
import { processingStageLabel } from '../../utils/stages';
// Returns: "Transcribing microphone audio", "Merging transcript channels", etc.
processingStageLabel('TranscribingMic');
```

Used by RecordingView, TranscriptPanel, and BackgroundTaskPill.

### `src/utils/format.ts`

Formatting utilities: `formatDuration`, `formatRelativeDate`, `formatBytes`.

### `src/lib/utils.ts`

The `cn()` utility (clsx + tailwind-merge) for conditional class composition:

```tsx
import { cn } from '../../lib/utils';
<div className={cn('base-classes', isActive && 'active-classes', className)} />
```

---

## 11. Z-Index Layers

| Layer | Z-Index | Component |
|---|---|---|
| Overlays/Modals | `z-50` | Modal backdrop, Toasts |
| Navigation (mobile) | `z-40` | Mobile bottom nav |
| Background tasks | `z-40` | BackgroundTaskPill |
| Dropdown menus | `z-20` | MeetingCard menu |
| Dropdown backdrop | `z-10` | Click-away overlay |

### Positioning Rules

- **Toasts**: `fixed top-4 right-4 z-50`
- **BackgroundTaskPill**: `fixed bottom-20 right-4 md:bottom-4 z-40` (avoids mobile nav and toast)
- **Mobile nav**: `fixed inset-x-0 bottom-3 z-40`

---

## 12. File Organization

```
src/components/
├── ui/              # Reusable primitives — no business logic
│   ├── Badge.tsx
│   ├── Button.tsx
│   ├── Card.tsx
│   ├── EmptyState.tsx
│   ├── Input.tsx
│   ├── Modal.tsx
│   ├── Progress.tsx
│   ├── Select.tsx
│   ├── Skeleton.tsx
│   ├── Tabs.tsx
│   ├── Toast.tsx
│   ├── Toggle.tsx
│   ├── BackgroundTaskPill.tsx
│   └── index.ts     # Barrel re-exports
├── Chat/            # Feature: RAG chat interface
├── Library/         # Feature: meeting library/list
├── Meeting/         # Feature: meeting detail (transcript, summary, notes)
├── Recording/       # Feature: recording view
├── Settings/        # Feature: settings views
└── Layout.tsx       # App shell (sidebar + nav)
```

### Conventions

- Each feature folder has a main `*View.tsx` component (e.g. `ChatView.tsx`, `RecordingView.tsx`)
- Sub-components are co-located in the feature folder
- Feature folders may have an `index.ts` for public exports
- UI primitives never import from feature components
- Feature components import from `../ui/` for primitives

---

## 13. Checklist for New Components

When building a new feature component:

1. **Colors**: Use only semantic tokens (`brand`, `success`, `warning`, `highlight`, etc.). No raw palette.
2. **Typography**: Lean on the global h2/h3 base styles. Use `text-sm` for body, `text-xs` for metadata.
3. **Borders**: Always `border-border`. Tinted borders: `border-brand/20`, `border-destructive/30`.
4. **Cards**: Use `<Card>` component, not manual div+border+shadow.
5. **Inputs**: Use `<Select>`, `<Input>`, `<Textarea>`, `<Toggle>` — never raw HTML.
6. **Badges**: Use `<Badge variant="...">` — never manual colored spans.
7. **Progress**: Use `<ProgressBar>` — single fill color, no color prop.
8. **Empty states**: Use `<EmptyState>` with icon, title, description, optional action.
9. **Icons**: Lucide only. Match sizing to context (see section 5).
10. **Dark mode**: Don't use `dark:` prefix. Tokens handle it.
11. **Keyboard**: Dropdown menus should close on Escape. Modals already do.
12. **Processing stages**: Use `processingStageLabel()` from `utils/stages.ts`.
