# Marketing Site Enhancement Design

**Date:** 2026-06-01  
**Project:** dictate marketing site  
**Approach:** Progressive Enhancement

## Overview

Enhance the existing dictate marketing site to create a premium, polished experience while maintaining the current Sage & Stone aesthetic and content structure. Focus on typography refinement, GSAP animations, and layout improvements to achieve a balanced presentation that makes installation easy while showcasing technical sophistication.

## Design Goals

1. **Balanced messaging**: Easy installation + technical excellence
2. **Visual consistency**: Keep Sage & Stone warm earthy aesthetic
3. **Premium feel**: Award-winning design execution through motion and spacing
4. **Accessibility**: Maintain or improve current accessibility standards
5. **Performance**: Animations should be smooth and non-blocking

## Typography & Spacing System

### Typography Scale

**Hero H1:**
- Font size: `clamp(3rem, 5vw, 5.5rem)`
- Container: `max-w-5xl` or `max-w-6xl` (up from current `max-w-680px`)
- Line height: `1.1`
- Letter spacing: `-0.02em`
- Maximum 2-3 lines of text
- Accent color on `<span>` elements (sage green)

**Section Headings (H2):**
- Font size: `clamp(1.8rem, 3vw, 2.4rem)`
- Font weight: `700`
- Letter spacing: `-0.02em`
- Margin bottom: `clamp(28px, 4vh, 44px)`

**Body Text:**
- Keep current comfortable size (`15px` base)
- Line height: `1.6`
- Color: `var(--text2)` for secondary text

**Font Stack:**
- Primary: Inter (already loaded)
- Monospace: JetBrains Mono / Fira Code (already defined)

### Spacing System

**Section Padding:**
- Vertical: `py-32 md:py-48` (up from current `py-16/py-20`)
- Horizontal: Keep current `px-8` (32px via `.wrap`)
- Hero specific: `pt-32 pb-24 md:pt-40 md:pb-32`

**Container Widths:**
- Main content: `max-w-1000px` (keep current)
- Hero H1: `max-w-5xl` or `max-w-6xl`
- Section descriptions: `max-w-440px` (keep current)

**Rationale:**
- Wider hero container prevents ugly 5-6 line text wrapping
- Massive vertical spacing creates distinct visual chapters
- Fluid typography scales beautifully across devices

## Hero Section Enhancement

### Structure

**Current elements to keep:**
- Agent prompt as primary CTA (unique differentiator)
- Manual install command as secondary option
- "or" divider between CTAs
- Copy buttons for both options
- Agent badges (Claude Code, Cursor, etc.)

**Changes:**

1. **Remove emoji tags** (🦀 Rust, 🔊 PipeWire, 🐧 Wayland)
   - Replace with clean text badges: "Rust", "PipeWire", "Wayland"
   - Style: small, subtle, sage-tinted background

2. **Widen H1 container**
   - Change from `max-w-680px` to `max-w-5xl`
   - Ensures 2-3 line maximum

3. **Visual hierarchy**
   - H1 with accent color on "terminal" span
   - Larger description text
   - Agent prompt block as hero element (already prominent)
   - Manual install below divider

4. **Background treatment**
   - Subtle radial gradient in sage tones
   - Or: mesh blur effect with warm colors
   - Keep it subtle to maintain readability

### Layout

```
[Clean text badges: Rust | PipeWire | Wayland]

[H1: Voice to text, from your terminal.]
  (max-w-5xl, 2-3 lines)

[Description paragraph]

[Agent prompt section - primary CTA]
  - Label
  - Prompt block with copy button
  - Agent badges

[--- or divider ---]

[Manual install command with copy button]
```

## GSAP Animations & Motion

### Dependencies

Add to `package.json`:
```json
{
  "gsap": "^3.12.5",
  "@gsap/react": "^2.1.1"
}
```

### Animation Patterns

**Hero entrance (page load):**
- Stagger animation with 100-150ms delays
- Sequence: badges → H1 → description → prompt block → install command
- Effect: fade in + slide up (20px)
- Easing: `power2.out`
- Duration: 0.8s per element

**Scroll reveal (sections entering viewport):**
- Section headings: fade in + scale (0.95 → 1.0)
- Trigger: when 20% of element enters viewport
- Duration: 0.6s
- Easing: `power2.out`

**Flow cards (How it works section):**
- Stagger animation: each card animates sequentially
- Effect: fade in + slide up (30px)
- Delay between cards: 150ms
- Trigger: when section enters viewport

**Bento grid (Features section):**
- Stagger animation with slight rotation
- Effect: fade in + scale (0.95 → 1.0) + rotate (2deg → 0deg)
- Delay between cards: 100ms
- Trigger: when section enters viewport

**Reference grid:**
- Simple fade in with stagger
- Delay between items: 80ms

### Hover Physics

**All cards:**
```css
transition: transform 0.7s ease-out;
```
On hover: `transform: scale(1.02)`

**Card images:**
- Wrap in `overflow-hidden` containers
- Image scales to `1.05` on card hover

**Copy buttons:**
- Scale to `1.05` on hover
- Background color shift (lighter sage)
- Transition: `0.2s ease`

**Nav CTA:**
- Shadow expansion on hover
- Slight scale: `1.02`

### Advanced Scroll Effects

**Pinned section (How it works):**
- Pin section heading on left while flow cards scroll up on right
- Use ScrollTrigger `pin: true`
- Only on desktop (md breakpoint and up)

**Feature cards scale:**
- Cards start at `scale(0.95)` when below viewport
- Animate to `scale(1.0)` as they enter
- Smooth scrubbing with ScrollTrigger

**Hero parallax (optional):**
- Background elements move slower than foreground
- Subtle effect, 0.3-0.5 speed multiplier

### Implementation Notes

- Use `useGSAP` hook from `@gsap/react` for proper cleanup
- Register ScrollTrigger plugin
- Ensure animations respect `prefers-reduced-motion`
- All animations should be GPU-accelerated (transform, opacity only)

## Layout & Grid Refinements

### Bento Grid (Features Section)

**Current structure:** 7 feature cards in CSS grid

**Enhancements:**
1. Add `grid-auto-flow: dense` (Tailwind: `grid-flow-dense`)
   - Eliminates empty gaps in grid
   - Cards fill available space intelligently

2. Verify column spans create perfect layout
   - No empty corners or voids
   - Cards interlock mathematically

3. Increase card padding
   - Internal padding: `p-6 md:p-8` (up from current)
   - Gap between cards: `gap-4 md:gap-6`

4. Hover effects
   - Lift: `translateY(-4px)`
   - Shadow: increase from `var(--shadow)` to `var(--shadow-lg)`
   - Transition: `0.3s ease`

### Flow Cards (How It Works)

**Current structure:** 4 cards in horizontal layout

**Enhancements:**
1. Increase spacing between cards
   - Gap: `gap-6 md:gap-8`

2. Add depth
   - Subtle gradient borders or shadows
   - Hover lift effect

3. Ensure consistent heights
   - Use flexbox or grid to align card bottoms

### Reference Grid

**Current structure:** Command reference items in grid

**Enhancements:**
1. Increase gap: `gap-6 md:gap-8`
2. Hover state for command blocks
   - Background color shift (lighter)
   - Border color change
3. Consistent copy button positioning

### Navigation

**Keep current implementation:**
- Sticky positioning
- Backdrop blur
- Border bottom

**Minor enhancements:**
- Increase height slightly: `64px` (from `60px`)
- Ensure smooth scroll to anchors works perfectly

### Footer

**Enhancements:**
- Increase padding: `py-16` (from current)
- Keep minimal design (appropriate)

### Page Structure

**Overflow prevention:**
```jsx
<main className="overflow-x-hidden w-full max-w-full">
  {/* all page content */}
</main>
```

**Section consistency:**
- All sections use `.wrap` for max-width
- All sections have `.sec-line` dividers (keep current)
- Consistent vertical rhythm with new spacing system

## Color Palette

**Keep existing Sage & Stone palette:**
- Background: `#f4f2ee`
- Card: `#ffffff`
- Text: `#2c2c28`
- Accent: `#4a7c59` (sage green)
- Accent light: `#6aad7a`

**No changes to color system** — it's already well-defined and cohesive.

## Component Updates

### Existing Components to Enhance

**ScrollReveal component:**
- Replace or enhance with GSAP ScrollTrigger
- Keep the same API if possible for minimal code changes
- Add stagger support

**CopyButton component:**
- Add hover scale animation
- Add success feedback animation (checkmark or color pulse)
- Keep existing functionality

**InstallSection component:**
- Apply new spacing system
- Add scroll reveal animations
- Keep content structure

**DemoTerminal component:**
- Keep as-is (already interactive)
- Add scroll reveal animation when entering viewport

**AudioVisualizer component:**
- Keep as-is
- Add scroll reveal animation

### New Components (if needed)

None required — all enhancements work with existing components.

## Accessibility

**Maintain current standards:**
- Semantic HTML structure
- ARIA labels where appropriate
- Keyboard navigation support

**Animation considerations:**
- Respect `prefers-reduced-motion` media query
- Disable GSAP animations when user prefers reduced motion
- Ensure all interactive elements remain accessible

**Color contrast:**
- Verify all text meets WCAG AA standards
- Sage accent on white background: already passes
- Keep current contrast ratios

## Performance

**Animation performance:**
- Use GPU-accelerated properties only (transform, opacity)
- Avoid animating layout properties (width, height, margin)
- Use `will-change` sparingly and only during animation

**GSAP bundle size:**
- Import only needed plugins (ScrollTrigger)
- Tree-shake unused features
- Total addition: ~30-40KB gzipped

**Loading strategy:**
- GSAP loads with main bundle (not critical path)
- Animations enhance experience but don't block content
- Progressive enhancement approach

## Implementation Order

1. **Install dependencies** (gsap, @gsap/react)
2. **Update spacing system** in globals.css
3. **Enhance hero section** (typography, layout, badges)
4. **Add GSAP animations** (hero entrance, scroll reveals)
5. **Refine bento grid** (grid-flow-dense, hover effects)
6. **Add hover physics** to all interactive elements
7. **Test animations** across devices and browsers
8. **Verify accessibility** (reduced motion, keyboard nav)
9. **Performance audit** (animation smoothness, bundle size)

## Success Criteria

**Visual quality:**
- Hero makes strong first impression
- Animations feel smooth and premium
- Spacing creates clear visual hierarchy
- No horizontal scroll issues

**Technical quality:**
- 60fps animations on modern devices
- Respects user motion preferences
- No layout shift or jank
- Bundle size increase < 50KB

**User experience:**
- Clear path to installation
- Technical sophistication communicated through design
- Sage & Stone aesthetic maintained
- All existing functionality preserved

## Out of Scope

- Content changes (keep existing copy)
- New sections or features
- Color palette changes
- Complete redesign
- Backend or API changes
- SEO optimization (separate effort)

## Notes

- This is a progressive enhancement — existing site remains functional throughout
- All changes are additive, not destructive
- Can be implemented incrementally
- Rollback is straightforward (remove GSAP, revert CSS)
