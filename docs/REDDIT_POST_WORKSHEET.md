# SnipExpand Reddit post worksheet

This is a writing scaffold, not a finished post. Fill it in with your own words
and remove every instruction, placeholder, and unused section before posting.

## Recommended first post

- Community: `r/omarchy`
- Flair: `I Made a Thing`
- Identity: post from your builder account and state plainly that you made it
- Goal: find early users and application-specific reliability problems
- Primary link: <https://github.com/silouanwright/snipexpand>
- Demo: use the GIF or MP4 already shown in the README

Check the community rules again immediately before posting. The current
`r/omarchy` rule permits useful Omarchy projects in moderation, asks promoters
to use the `I Made a Thing` flair, and expects participation beyond dropping a
link.

## Decide what you actually want

Complete these before writing:

- The one outcome I want from this post: **[TESTERS / FEEDBACK / CONTRIBUTORS /
  AWARENESS]**
- The kind of person I most want to hear from: **[DESCRIBE THEM]**
- The one question I want comments to answer: **[SPECIFIC QUESTION]**
- The most honest reason I built this: **[ONE OR TWO SENTENCES]**
- The moment I realized existing tools were not enough: **[SPECIFIC EXPERIENCE]**
- The limitation I am most interested in testing: **[APPLICATION / KEYBOARD
  LAYOUT / WORKFLOW]**

If you cannot answer these concretely, wait before drafting. A post written only
to collect clicks will read like one.

## Title workshop

Write at least five titles, then select the clearest one. A reader should
understand the problem or use case without already knowing what SnipExpand is.

Useful shapes:

1. `I built [PLAIN DESCRIPTION] for [SPECIFIC PROBLEM] on Omarchy`
2. `I wanted [OUTCOME] on Omarchy, so I built [PLAIN DESCRIPTION]`
3. `SnipExpand: [WHAT IT DOES], built for Omarchy and Hyprland`
4. `I built a Rust text expander because [SPECIFIC WAYLAND PROBLEM]`
5. `[SPECIFIC PROBLEM] kept bothering me on Omarchy, so I built this`

Candidate titles:

1. **[TITLE]**
2. **[TITLE]**
3. **[TITLE]**
4. **[TITLE]**
5. **[TITLE]**

Avoid claims such as `best`, `revolutionary`, `game-changing`, `blazing fast`,
or `flawless`. Avoid stuffing every feature into the title.

## Post scaffold

### 1. Open with the problem and your connection to it

Use two or three short sentences. Write in first person. Be specific about what
was unreliable or awkward in your own workflow.

> **[WHAT YOU WANTED TO DO]**
>
> **[WHAT KEPT FAILING OR FELT AWKWARD]**
>
> **[WHY THAT LED YOU TO BUILD SOMETHING]**

Do not begin with a company introduction, a feature list, or a generic history
of text expansion.

### 2. Say what you made

Use one compact paragraph.

> I made SnipExpand, **[YOUR PLAIN-LANGUAGE DESCRIPTION]**. It **[THE MAIN
> RESULT FOR THE USER]**.

State explicitly that you are the developer. Do not pose as a user who happened
to discover the project.

### 3. Show it

Place the demo near the top, after readers understand what they are seeing.

> **[ATTACH THE GIF OR VIDEO NATIVELY IF THE COMMUNITY SUPPORTS IT]**

Optional one-sentence caption:

> **[WHAT HAPPENS IN THE DEMO, WITHOUT REPEATING THE ENTIRE POST]**

### 4. Explain what is meaningfully different

Choose three or four points that matter specifically to Omarchy users. Do not
copy the complete README feature list.

- **[OMARCHY OR HYPRLAND-SPECIFIC ADVANTAGE]**
- **[SYSTEM-WIDE OR CLIPBOARD-FREE BEHAVIOR]**
- **[CONFIGURATION OR HOT-RELOAD ADVANTAGE]**
- **[RUST / OPEN-SOURCE / LOCAL-ONLY ADVANTAGE, IF RELEVANT]**

Explain one interesting technical choice in ordinary language if the audience
would appreciate it:

> **[FOR EXAMPLE: WHY A PERSISTENT WAYLAND VIRTUAL KEYBOARD IS USED]**

### 5. Be candid about maturity and limitations

Pick the limitations most relevant to the community rather than hiding them.

> It is early, and **[CURRENT LIMITATION]**. So far I have tested **[WHAT YOU
> HAVE ACTUALLY TESTED]**. I especially want to learn **[WHAT REMAINS UNKNOWN]**.

This turns limitations into a credible request for feedback without pretending
the project is unfinished in every respect.

### 6. Make one specific request

Good requests are easy to answer:

- Which applications should I add to the compatibility matrix first?
- Does it behave correctly with your keyboard layout?
- Where does text expansion currently fail in your Omarchy workflow?
- What is the smallest missing feature that would make you use it daily?

Choose one primary question:

> **[YOUR QUESTION]**

### 7. End with the useful links

> GitHub: <https://github.com/silouanwright/snipexpand>
>
> Install: `cargo install snipexpand`
>
> **[OPTIONAL RELEASE LINK]**

Do not ask for stars, upvotes, follows, or artificial engagement. Ask people to
try it or critique it only if that is genuinely what you want.

## Verified fact bank

Use these facts selectively. Do not place all of them in one post.

- SnipExpand is a Rust text expander for Linux and Wayland.
- It was developed with first-class Omarchy and Hyprland support.
- Omarchy contains its required runtime dependencies by default.
- It performs system-wide expansion without using the clipboard.
- It supports immediate and terminator-based triggers.
- Replacements may contain plain text, multiline text, Unicode, and a final
  cursor marker.
- YAML match files reload when saved.
- It supports multiple triggers, word boundaries, case propagation, formatted
  date variables, application exclusions, and simple immediate undo.
- It uses persistent Wayland injection with a `uinput` fallback.
- It provides validation, diagnostics, daemon status, and application
  detection commands.
- It is free software under GPL-3.0-or-later.
- The repository currently has automated tests plus a byte-exact real Hyprland
  compatibility suite.
- SnipExpand does not execute snippets, read the clipboard, or contact online
  services.

## Claims that require careful wording

- Say `Hyprland is tested`, not `all Wayland compositors are supported`.
- Say `built for Omarchy and Hyprland`, not `works in every Linux application`.
- Say `clipboard-free expansion`, not `cannot expose sensitive input`.
- Application exclusions do not stop global input capture.
- Browser password fields cannot be detected independently of their browser.
- Undo is limited to the immediately preceding simple, single-line expansion.
- Arbitrary scripts, forms, rich text, and a package registry
  are not currently supported.
- If discussing Espanso, describe the specific Linux and Wayland problems that
  motivated you. Do not frame the post as an attack on its maintainers.

## Community-specific versions

Do not paste the same body into several communities.

### `r/omarchy`

Lead with the missing Omarchy experience, first-class setup, and a request for
testing across the default app ecosystem. Use the required self-promotion
flair.

### `r/hyprland`

Lead with reliable Wayland injection and the technical constraints that made
text expansion difficult. Ask about applications, layouts, and compositor
behavior.

### `r/linux`

Wait until the builder account has participated meaningfully. Explain the
broader Linux problem and remain active in the comments. The community asks
people submitting their own work to contribute more than their own links.

### `r/rust`

Only post a version that is substantively about Rust. Discuss the input
pipeline, Wayland protocol, reliability testing, or a technical lesson from the
implementation. A generic product announcement is a poor fit.

### `r/opensource`

Expect account and karma requirements. Do not farm karma to bypass them. Focus
on the open-source problem, maintenance intentions, and the kind of
contribution or feedback you want.

## Using AI without losing your voice

The strongest use of AI here is as an editor after you write the raw draft.
Give it your actual experience, awkward sentences, opinions, and technical
details. Those are the parts that make the post sound like you.

Recommended editing prompt:

> Edit this Reddit draft conservatively. Preserve my first-person voice,
> opinions, uncertainty, and technical specificity. Remove repetition and
> marketing language. Do not invent experiences, metrics, user quotes, or
> claims. Do not add a dramatic hook, fake vulnerability, corporate language,
> emojis, an engagement-bait question, or em dashes. Point out anything that
> sounds promotional or unsupported, but ask before making a substantial
> rewrite.

Useful AI review questions:

- Which sentence sounds most like marketing copy?
- Which claim needs evidence or narrower wording?
- What would a skeptical Omarchy user question first?
- What can be removed without losing the story?
- Does the title clearly communicate a use case?
- Does the post still offer value if the project link is removed?

Do not use AI to manufacture a question and later reply as though you discovered
your own product. Do not invent a customer story, adoption number, failure, or
quote. Those tactics are astroturfing, not authentic writing.

## Final human checklist

- [ ] I checked the community rules immediately before posting.
- [ ] I used the correct self-promotion flair.
- [ ] The title communicates one problem or result.
- [ ] The first paragraph contains a specific personal reason for building it.
- [ ] I clearly disclosed that I made SnipExpand.
- [ ] The demo appears early and is understandable without sound.
- [ ] I selected only the most relevant features.
- [ ] Every technical claim is supported by the repository or my experience.
- [ ] I named a real limitation.
- [ ] I asked one specific question.
- [ ] I removed requests for stars, upvotes, and follows.
- [ ] I removed generic AI and marketing phrasing.
- [ ] I can remain available to answer comments after posting.
- [ ] I did not use another account to vote or comment on the post.

## Research notes

- [Reddit spam guidance](https://support.reddithelp.com/hc/en-us/articles/360043504051-Spam)
  recommends authentic participation and warns against accounts whose activity
  primarily promotes something they own.
- [`r/omarchy` self-promotion rule](https://www.reddit.com/r/omarchy/comments/1uofb4s/removed/)
  permits useful Omarchy projects in moderation and requires the `I Made a
  Thing` flair.
- [`r/linux` community guidance](https://es.reddit.com/r/linux/comments/19b0asa/most_deadly_linux_commands/)
  welcomes original work while asking authors to contribute beyond their own
  links and engage with comments.
- [Open-source promotion discussion](https://www.reddit.com/r/opensource/comments/1ixgfap/as_a_foss_dev_im_torn_about_promoting_my_projects/)
  repeatedly favors transparent authorship, narrowly targeted communities,
  useful technical context, and genuine participation.
- [A recent successful Rust project post](https://www.reddit.com/r/rust/comments/1vunsq3/i_built_a_weird_rust_compiler_that_runs_entirely/)
  quickly explains the use case, then gives enough implementation detail to
  make the post interesting independently of its link.
- [Reddit's guidance for working with moderators](https://www.business.reddit.com/learning-hub/articles/how-to-work-with-moderators-on-reddit)
  emphasizes checking community rules and belonging rather than broadcasting.
