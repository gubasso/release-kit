# Skill Format Sources

External sources behind the rules of `SPEC-skill-format.md`, and behind the package shape `distribution:shared-skill-artifacts-have-one-home` states: what the skill format, the vendors, and the research say about how a skill is packaged, loaded, sized, and written, and how the tools that install skills lay out a package. Decision records state what was chosen; this file states what the choice was checked against.

Verified against the listed sources on 2026-10-08, and the sources under "Installing a skill package" on 2026-10-09. Every source was read again as raw text on 2026-10-10.

Each entry names its URL, its date where the source states one, the finding, and a short quote where the wording matters. A date in parentheses is the publication date.

## Contents

- Specifications and vendor guidance for skills
- Context engineering
- Context windows
- Long-context degradation
- Instruction count and compliance
- Prompt format
- Token proxies and the measurement
- Installing a skill package
- Where the sources conflict
- What the research could not verify

## Specifications and vendor guidance for skills

### Agent Skills specification

- URL: <https://agentskills.io/specification>. Source repository: <https://github.com/agentskills/agentskills>, file `docs/specification.mdx`.
- Layout: a required `SKILL.md`, and optional `scripts/`, `references/`, and `assets/`.
- Frontmatter, hard limits:
  - `name`: 1 to 64 characters, lowercase letters, digits, and hyphens. It has no leading, trailing, or double hyphen, and it matches the parent directory.
  - `description`: 1 to 1,024 characters. It "Should describe both what the skill does and when to use it".
  - `compatibility`: at most 500 characters. "Most skills do not need the `compatibility` field."
  - `license`, `metadata` (a string-to-string map), and `allowed-tools` (experimental) are optional.
- Body: "There are no format restrictions." The recommended contents are step-by-step instructions, examples, and edge cases.
- Progressive disclosure: metadata takes about 100 tokens at startup. The body takes "< 5000 tokens recommended" on activation. Resources load "only when required".
- Size: "Keep your main `SKILL.md` under 500 lines. Move detailed reference material to separate files." Both figures are recommendations.
- References: "use relative paths from the skill root". "Keep file references one level deep from `SKILL.md`. Avoid deeply nested reference chains."
- Reference files: "Keep individual reference files focused. Agents load these on demand, so smaller files mean less use of context."
- Validation: `skills-ref validate` checks the frontmatter only. The skills-ref README says the library "is intended for demonstration purposes only".

Bearing: `skill-format:a-package-holds-the-manual-and-its-references`, `skill-format:the-manual-is-the-control-plane`, `skill-format:the-manual-links-every-reference-directly`, `skill-format:shared-content-is-authored-once`, `skill-format:the-description-says-what-then-when`, `distribution:shared-skill-artifacts-have-one-home`, and the size of a skill body.

### Agent Skills: client implementation guide

- URL: <https://agentskills.io/client-implementation/adding-skills-support>
- The catalog costs "~50-100 tokens per skill".
- `.agents/skills/` at project and user scope "have emerged as a widely-adopted convention for cross-client skill sharing".
- Clients should "exempt skill content from pruning" during compaction.

Bearing: `distribution:a-skill-has-one-owner`, through the `.agents/skills` convention.

### Agent Skills: best practices

- URL: <https://agentskills.io/skill-creation/best-practices>
- "Concise, stepwise guidance with a working example tends to outperform exhaustive documentation."
- The deletion test: "Would the agent get this wrong without this instruction? If the answer is no, cut it."
- "Provide defaults, not menus."
- "Favor procedures over declarations."
- "The highest-value content in many skills is a list of gotchas." Keep the gotchas in `SKILL.md`.
- Templates are "more reliable than describing the format in prose, because agents pattern-match well against concrete structures".
- A link carries its loading condition: "Read `references/api-errors.md` if the API returns a non-200 status code" works better than "see references/".

Bearing: `skill-format:a-reference-link-carries-its-condition`, `skill-format:a-choice-offers-one-default`, `skill-format:every-line-passes-the-deletion-test`.

### Agent Skills: evaluating and optimizing

- URLs: <https://agentskills.io/skill-creation/evaluating-skills> and <https://agentskills.io/skill-creation/optimizing-descriptions>
- "Reasoning-based instructions ('Do X because Y tends to cause Z') work better than rigid directives ('ALWAYS do X, NEVER do Y')."
- Evaluation: run output scenarios with and without the skill. Test about 20 trigger queries, 8 to 10 that should trigger and 8 to 10 that should not, with near misses as the most valuable negatives, split 60/40 into training and validation.

Bearing: `skill-format:prose-is-only-a-reason-that-changes-behavior`, `skill-format:a-hard-rule-is-a-check-not-capitals`.

### Claude: skill authoring best practices

- URL: <https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices>
- "The context window is a public good."
- "Default assumption: Claude is already very smart. Only add context Claude doesn't already have."
- Test each paragraph: "Does this paragraph justify its token cost?"
- Degrees of freedom: "Match the level of specificity to the task's fragility and variability." Use exact commands for fragile steps and heuristics where many approaches work.
- "Keep SKILL.md body under 500 lines for optimal performance." This is a recommendation.
- "Keep references one level deep from SKILL.md. All reference files should link directly from SKILL.md." The reason: Claude "might use commands like `head -100`" on a nested file.
- "For reference files longer than 100 lines, include a table of contents at the top."
- "Break complex operations into clear, sequential steps." Complex workflows get a checklist the agent copies and checks off.
- Feedback loops: "Run validator → fix errors → repeat."
- "Choose one term and use it throughout the Skill."
- No time-sensitive information. Legacy material goes into a labeled "Old patterns" section.
- Descriptions: "Always write in third person." Include what the skill does and when to use it.
- "Prefer scripts for deterministic operations." Say whether the agent runs a script or reads it.
- "Create evaluations BEFORE writing extensive documentation." "There is not currently a built-in way to run these evaluations."
- `name` and `description` carry no XML tags, and `name` carries no "anthropic" or "claude".

Bearing: `skill-format:the-manual-links-every-reference-directly`, `skill-format:one-term-names-one-concept`, `skill-format:the-description-says-what-then-when`, and the list of sections a long reference file opens with.

### Claude: Agent Skills overview

- URL: <https://platform.claude.com/docs/en/agents-and-tools/agent-skills/overview>
- Loading levels: metadata "~100 tokens per Skill", instructions "Under 5k tokens", resources "None until accessed".
- "Scripts run through bash, and only their output enters context."

Bearing: `skill-format:the-manual-is-the-control-plane`, and the size of a skill body.

### Claude Code: skills

- URL: <https://code.claude.com/docs/en/skills>
- "Keep `SKILL.md` under 500 lines. Move detailed reference material to separate files."
- "Keep the body itself concise. Once a skill loads, its content stays in context across turns, so every line is a recurring token cost. State what to do rather than narrating how or why."
- "Claude Code does not re-read the skill file on later turns, so write guidance that should apply throughout a task as standing instructions rather than one-time steps."
- After compaction, Claude Code re-attaches "the first 5,000 tokens of each" skill, within a "combined budget of 25,000 tokens". The page therefore advises putting the most important instructions near the top.
- The listing truncates the combined `description` and `when_to_use` at 1,536 characters. The listing budget scales at 1% of the context window.
- A rule that must hold every time moves into a hook.
- Only the six specification fields work outside Claude Code.

Bearing: `skill-format:the-manual-is-the-control-plane`, `skill-format:the-manual-follows-the-stage-order`, `skill-format:prose-is-only-a-reason-that-changes-behavior`, `skill-format:the-rule-that-matters-most-comes-first`, `skill-format:a-hard-rule-is-a-check-not-capitals`, and the size of a skill body.

### Claude Code: memory and best practices

- URLs: <https://code.claude.com/docs/en/memory> and <https://code.claude.com/docs/en/best-practices>
- "target under 200 lines per CLAUDE.md file. Longer files consume more context and reduce adherence."
- "group related instructions under markdown headers and bullets. Organized sections are easier for Claude to follow than dense paragraphs."
- Write instructions "concrete enough to verify": "Use 2-space indentation", not "Format code properly".
- "if two instructions contradict each other, Claude may pick one arbitrarily."
- "Bloated CLAUDE.md files cause Claude to ignore your actual instructions!"
- "If you emphasize many lines, none of them stands out."
- "For each line, ask: 'Would removing this cause Claude to make mistakes?' If not, cut it."

Bearing: `skill-format:a-fact-lives-in-one-file`, `skill-format:an-instruction-is-one-checkable-action`, `skill-format:the-structure-follows-the-content`, `skill-format:every-line-passes-the-deletion-test`.

### Anthropic: Equipping agents for the real world with Agent Skills

- URL: <https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills> (2025-10-16, update note dated 2025-12-18)
- Skills are "organized folders of instructions, scripts, and resources that agents can discover and load dynamically".
- Practices: start with evaluation, split an unwieldy `SKILL.md`, keep mutually exclusive contexts in separate files, and iterate with the model.
- The post states no numeric size limit.

Bearing: `skill-format:the-manual-is-the-control-plane`.

### Anthropic: skill-creator skill

- URL: <https://github.com/anthropics/skills/blob/main/skills/skill-creator/SKILL.md>
- "Prefer using the imperative form in instructions."
- All "when to use" information goes in the description, not the body.
- "Try to explain to the model why things are important in lieu of heavy-handed musty MUSTs."
- "If you find yourself writing ALWAYS or NEVER in all caps … that's a yellow flag."
- "Keep the prompt lean. Remove things that aren't pulling their weight."
- For reference files over 300 lines, include a table of contents. This threshold conflicts with the 100 lines on the best-practices page.
- In the same repository, `claude-api/SKILL.md` has 616 lines at commit `dbd4588` (2026-10-09). The 500-line figure is not enforced.

Bearing: `skill-format:an-instruction-is-one-checkable-action`, `skill-format:a-hard-rule-is-a-check-not-capitals`, and the list of sections a long reference file opens with.

### Anthropic: The Complete Guide to Building Skills for Claude

- URL: <https://resources.anthropic.com/hubfs/The-Complete-Guide-to-Building-Skill-for-Claude.pdf> (2026-01)
- "Keep SKILL.md under 5,000 words." This is a third unit beside lines and tokens.
- "Don't include README.md inside your skill folder."
- Description: "[What it does] + [When to use it] + [Key capabilities]". Add negative triggers where an adjacent skill exists.
- Be specific and actionable: "Run `python scripts/validate.py --input {filename}`", not "Validate the data before proceeding."
- "Put critical instructions at the top." "Use bullet points and numbered lists."
- "Code is deterministic; language interpretation isn't."

Bearing: `skill-format:a-package-holds-the-manual-and-its-references`, `skill-format:the-rule-that-matters-most-comes-first`, and the size of a skill body.

### OpenAI: skills in Codex and ChatGPT

- URL: <https://learn.chatgpt.com/docs/build-skills>
- "Keep each skill focused on one job."
- "Prefer instructions over scripts unless you need deterministic behavior or external tooling."
- "Write imperative steps with explicit inputs and outputs."
- The skill list takes "at most 2% of the model's context window, or 8,000 characters when the context window is unknown".
- "Front-load the key use case and trigger words so a host can still match the skill if descriptions are shortened."

Bearing: `skill-format:an-instruction-is-one-checkable-action`, `skill-format:the-description-says-what-then-when`.

### OpenAI: skill-creator skill

- URL: <https://github.com/openai/skills/blob/main/skills/.system/skill-creator/SKILL.md>
- "Always use imperative/infinitive form."
- "Information should live in either SKILL.md or references files, not both."
- Do not create a README, an install guide, a quick reference, or a changelog inside a skill.
- A body under "<5k words" and 500 lines. A table of contents for reference files over 100 lines.

Bearing: `skill-format:a-package-holds-the-manual-and-its-references`, `skill-format:a-fact-lives-in-one-file`, and the size of a skill body.

### OpenAI: AGENTS.md in Codex

- URL: <https://learn.chatgpt.com/docs/agent-configuration/agents-md>
- Codex "stops adding files once the combined size reaches the limit defined by project_doc_max_bytes (32 KiB by default)".

Bearing: the size of a reference file.

### Other adopters

- GitHub Copilot: <https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/customize-cloud-agent/add-skills>. Use "custom instructions for simple instructions relevant to almost every task" and "skills for more detailed instructions that Copilot should only access when relevant."
- VS Code: <https://code.visualstudio.com/docs/agent-customization/agent-skills>. "Names with invalid characters cause the skill to silently fail to load."
- Cursor: <https://cursor.com/docs/skills>. It reads `.agents/skills/`, `.cursor/skills/`, `.claude/skills/`, and `.codex/skills/`.
- Gemini CLI: <https://geminicli.com/docs/cli/skills/> and <https://geminicli.com/docs/cli/creating-skills/>. The description "is how Gemini decides when to use the skill". No size limit is stated.
- AGENTS.md: <https://agents.md/>. "just standard Markdown. Use any headings you like". No size guidance.

Bearing: `distribution:a-skill-has-one-owner` and the two agent roots the installer writes.

## Context engineering

### Anthropic: Effective context engineering for AI agents

- URL: <https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents> (2025-09-29)
- Good context engineering means "finding the smallest possible set of high-signal tokens that maximize the likelihood of some desired outcome."
- "Context, therefore, must be treated as a finite resource with diminishing marginal returns." Degradation is "a performance gradient rather than a hard cliff".
- The "right altitude" sits between two failure modes. One is hardcoded brittle logic. The other is vague guidance that "fails to give the LLM concrete signals".
- "minimal does not necessarily mean short".
- Warns against "a laundry list of edge cases". Prefers "diverse, canonical examples".
- Just-in-time loading keeps "lightweight identifiers (file paths, stored queries, web links, etc.)" and loads the content at runtime.
- "the exact formatting of prompts is likely becoming less important as models become more capable."

Bearing: `skill-format:every-line-passes-the-deletion-test`, `skill-format:the-manual-is-the-control-plane`.

## Context windows

Bearing: the size of a skill body: a larger window does not remove the cost of the text a skill loads.

- Claude: <https://platform.claude.com/docs/en/docs/about-claude/models/overview> and <https://platform.claude.com/docs/en/build-with-claude/context-windows>. Every current model has a 1M-token window by default. The same page warns: "more context isn't automatically better. As token count grows, accuracy and recall degrade, a phenomenon known as context rot."
- Claude pricing: <https://platform.claude.com/docs/en/about-claude/pricing>. "A 900k-token request is billed at the same per-token rate as a 9k-token request", for Claude 4.6 and later models except Haiku 5.5.
- OpenAI: <https://developers.openai.com/api/docs/models>. Current models list about 1.05M tokens. Prompts above 272K input tokens cost twice as much per input token ([pricing](https://developers.openai.com/api/docs/pricing)).
- Google: <https://ai.google.dev/gemini-api/docs/models/gemini-3.1-pro-preview>. 1,048,576 input tokens. Prompts above 200K tokens cost twice as much ([pricing](https://ai.google.dev/gemini-api/docs/pricing)).
- Google, long context: <https://ai.google.dev/gemini-api/docs/long-context>. "if you don't need tokens to be passed to the model, it is best to avoid passing them."
- Anthropic, Claude Opus 4.6 announcement: <https://www.anthropic.com/news/claude-opus-4-6> (2026-02-05). On an 8-needle retrieval test at 1M tokens, "Opus 4.6 scores 76%, whereas Sonnet 4.5 scores just 18.5%." Use of the full window varies widely between models.

## Long-context degradation

Bearing: `skill-format:the-manual-is-the-control-plane`, `skill-format:every-line-passes-the-deletion-test`, and the size of a skill body.

| Source                                                                                                       | Finding                                                                                                                                                                                |
| ------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Liu et al., "Lost in the Middle", TACL. <https://arxiv.org/abs/2307.03172> (2023-07-06)                      | Performance "significantly degrades when models must access relevant information in the middle of long contexts, even for explicitly long-context models."                             |
| Hsieh et al., RULER, COLM 2024. <https://arxiv.org/abs/2404.06654> (2024-04-09)                              | "only half of them can maintain satisfactory performance at the length of 32K."                                                                                                        |
| Modarressi et al., NoLiMa, ICML 2025. <https://arxiv.org/abs/2502.05167> (2025-02-07)                        | "At 32K, for instance, 11 models drop below 50% of their strong short-length baselines."                                                                                               |
| Hong, Troynikov, Huber, "Context Rot", Chroma. <https://www.trychroma.com/research/context-rot> (2025-07-14) | "Even a single distractor reduces performance relative to the baseline." Focused prompts scored "significantly higher" than full prompts.                                              |
| Shi et al., ICML 2023. <https://arxiv.org/abs/2302.00093>                                                    | "Model performance is dramatically decreased when irrelevant information is included."                                                                                                 |
| Levy, Jacoby, Goldberg, ACL 2024. <https://arxiv.org/abs/2402.14848>                                         | Reasoning degrades "at much shorter input lengths than their technical maximum."                                                                                                       |
| Du et al., Findings of EMNLP 2025. <https://arxiv.org/abs/2510.05381> (2025-10-06)                           | "Even when models can perfectly retrieve all relevant information, their performance still degrades substantially (13.9%–85%) as input length increases." Length alone costs accuracy. |
| Bai et al., LongBench v2. <https://arxiv.org/abs/2412.15204> (2024-12-19)                                    | The best direct-answer model "achieves only 50.1% accuracy."                                                                                                                           |
| Paulsen, "Maximum Effective Context Window". <https://arxiv.org/abs/2509.21361>                              | The effective window is far below the advertised one. This is weak evidence: one author, and an unusual method.                                                                        |

## Instruction count and compliance

Bearing: `skill-format:the-rule-that-matters-most-comes-first`, `skill-format:every-line-passes-the-deletion-test`, `skill-format:the-manual-follows-the-stage-order`.

| Source                                                                           | Finding                                                                                                                                                                                                                                  |
| -------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Zhou et al., IFEval. <https://arxiv.org/abs/2311.07911> (2023-11-14)             | The base benchmark for verifiable instructions.                                                                                                                                                                                          |
| Jiang et al., FollowBench, ACL 2024. <https://arxiv.org/abs/2310.20410>          | Compliance falls as constraints accumulate.                                                                                                                                                                                              |
| Jaroslawicz et al., IFScale. <https://arxiv.org/abs/2507.11538> (2025-07-15)     | "even the best frontier models only achieve 68% accuracy at the max density of 500 instructions". Models show a "bias towards earlier instructions".                                                                                     |
| Harada et al., ManyIFEval. <https://arxiv.org/abs/2509.21051> (2025-09-25)       | "performance consistently degrades as the number of instructions increases".                                                                                                                                                             |
| Qi et al., AgentIF. <https://arxiv.org/abs/2505.16944>                           | Real agent system prompts average "1,723 words" and "11.9 constraints per instruction". Current models "generally perform poorly".                                                                                                       |
| Ding et al., OctoBench. <https://arxiv.org/abs/2601.10343> (2026-01-15)          | "a systematic gap between task-solving and scaffold-aware compliance", across system prompts, repository files, and skills.                                                                                                              |
| Gloaguen et al., "Evaluating AGENTS.md". <https://arxiv.org/abs/2602.11988>      | Context files "do not generally improve task success rates, while increasing inference cost by over 20% on average". Repository overviews "are not helpful".                                                                             |
| Li et al., SkillsBench. <https://arxiv.org/abs/2602.12670>                       | "Focused Skills with at most three modules outperform larger or exhaustive bundles". In v4 (2026-06-14), compact skills gained 19.0 points and comprehensive ones 0.7, and the median `SKILL.md` is 4.8 KB. v1 to v3 give other figures. |
| Eliav, "Prompt Design at Scale". <https://arxiv.org/abs/2607.19257> (2026-07-21) | With 80 or more rules, the rate of responses that satisfy every rule is near zero for every model and format. Not peer reviewed.                                                                                                         |

## Prompt format

Bearing: `skill-format:the-structure-follows-the-content`, `skill-format:prose-is-only-a-reason-that-changes-behavior`, `skill-format:a-hard-rule-is-a-check-not-capitals`.

| Source                                                                                                                                      | Finding                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| He et al. <https://arxiv.org/abs/2411.10541> (2024-11-15)                                                                                   | "there is no universally optimal format, even within the same generational lineage." Larger models are more robust to format.                                                                                                                                                                                                                                                                                                                                    |
| Sclar et al., ICLR 2024. <https://arxiv.org/abs/2310.11324>                                                                                 | Spurious format changes moved accuracy by "up to 76 accuracy points" on a 13B model.                                                                                                                                                                                                                                                                                                                                                                             |
| Eliav, VeyraBench. <https://arxiv.org/abs/2607.19257>                                                                                       | "no model shows a consistent, meaningful markdown advantage". Markdown costs about 26% more tokens than plain text, and tables about 37% more.                                                                                                                                                                                                                                                                                                                   |
| Claude prompting best practices. <https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices> | "Provide instructions as sequential steps using numbered lists or bullet points when the order or completeness of steps matters." Explain the reason behind a rule: "Claude is smart enough to generalize from the explanation." Replace "CRITICAL: You MUST use this tool when…" with "Use this tool when…", because Claude Opus 4.5 and 4.6 over-trigger on aggressive wording written for earlier models. "Tell Claude what to do instead of what not to do." |
| Prompting Claude Sonnet 5. <https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-sonnet-5>             | The model "interprets prompts literally and explicitly... It does not silently generalize an instruction from one item to another." State each rule's scope.                                                                                                                                                                                                                                                                                                     |
| OpenAI GPT-4.1 prompting guide. <https://developers.openai.com/cookbook/examples/gpt4-1_prompting_guide> (2025-04-14)                       | Markdown sections are recommended. The model follows instructions "more literally". In a conflict, it follows the instruction closer to the end.                                                                                                                                                                                                                                                                                                                 |
| OpenAI GPT-5 prompting guide. <https://developers.openai.com/cookbook/examples/gpt-5/gpt-5_prompting_guide> (2025-08-07)                    | "contradictory or vague instructions can be more damaging".                                                                                                                                                                                                                                                                                                                                                                                                      |
| OpenAI, Using GPT-6. <https://developers.openai.com/api/docs/guides/latest-model>                                                           | "better able to follow longer instructions, but can also be more sensitive to information in context."                                                                                                                                                                                                                                                                                                                                                           |
| Google prompting strategies. <https://ai.google.dev/gemini-api/docs/prompting-strategies> (updated 2026-09-17)                              | "XML-style tags... or Markdown headings are effective". "Be precise and direct... Avoid unnecessary or overly persuasive language".                                                                                                                                                                                                                                                                                                                              |

## Token proxies and the measurement

Bearing: the unit a skill's size is counted in: characters, because the measurement below ranks them first.

Vendor rules of thumb:

- Anthropic models overview: <https://platform.claude.com/docs/en/docs/about-claude/models/overview>. "1M tokens is roughly 555k words or 2.5M Unicode characters on the current tokenizer (introduced with Claude Opus 4.7)". That is about 2.5 characters per token.
- Anthropic glossary: <https://platform.claude.com/docs/en/about-claude/glossary>. The earlier tokenizer gives "approximately 3.5 English characters" per token, and the newer one "approximately 30 percent more tokens for the same text".
- Anthropic token counting: <https://platform.claude.com/docs/en/build-with-claude/token-counting>. Token counting is free.
- Google: <https://ai.google.dev/gemini-api/docs/tokens>. "a token is equivalent to about 4 characters. 100 tokens is equal to about 60-80 English words."

Research:

- Roberts, Han, Albanie. <https://arxiv.org/abs/2601.11518> (2026-01-16). "token counts are not a stable unit of length." Compression ratios differ by 20% to 50% between tokenizers, and by 100% for emoji and numbers. The common rule of 0.75 words per token is "an oversimplification".
- Petrov et al., NeurIPS 2023. <https://arxiv.org/abs/2305.15425>. The same text in different languages differs in token length "up to 15 times".

The measurement, 2026-10-08:

- Method: the `tiktoken` tokenizers `o200k_base` and `cl100k_base` ran over two corpora. One pooled ratio per proxy predicted each file's token count. The error is the per-file error of that prediction. Claude's tokenizer is not public, so these figures come from OpenAI tokenizers.
- Corpus 1: 145 Markdown files of one documentation repository, written as unwrapped prose.
- Corpus 2: 90 skill files from the skill collections installed on one host.

| Corpus             | Proxy      | Correlation with tokens | Median error | 90th-percentile error | Worst error |
| ------------------ | ---------- | ----------------------- | ------------ | --------------------- | ----------- |
| Unwrapped Markdown | Characters | 0.989                   | 4%           | 12%                   | 39%         |
|                    | Words      | 0.965                   | 9%           | 19%                   | 42%         |
|                    | Lines      | 0.964                   | 12%          | 46%                   | 126%        |
| Skill files        | Characters | 0.995                   | 5%           | 9%                    | 12%         |
|                    | Words      | 0.971                   | 12%          | 21%                   | 27%         |
|                    | Lines      | 0.71                    | 27%          | 70%                   | 118%        |

- Tokens per line ranged from 6.5 to 34. A 500-line cap allows anything from about 3,000 to about 17,000 tokens.
- The Markdown averaged about 4.4 characters per token on these tokenizers. With the glossary's 30% increase, that is about 3.4 characters per token. Anthropic's 2.5 is the conservative bound, and the caps use it.

## Installing a skill package

Bearing: `skill-format:shared-content-is-authored-once`, `distribution:shared-skill-artifacts-have-one-home`, and `distribution:a-skill-destination-is-a-regular-file`.

Read on 2026-10-09. The question: where does a tool that installs its own skills put the content two skills share?

The format:

- Agent Skills specification. <https://agentskills.io/specification>. "A skill is a directory containing, at minimum, a `SKILL.md` file." `references/` holds "additional documentation that agents can read when needed". "When referencing other files in your skill, use relative paths from the skill root." "Keep file references one level deep from `SKILL.md`." The specification defines no location that two skills share.

Tools that install skills:

- Atlassian TWG CLI. <https://developer.atlassian.com/cloud/twg-cli/agents/skills/>. A CLI that installs the skills that operate it. "It always writes the canonical skill bundle to `~/.agents/skills`", and where Claude Code is installed, "Claude Code receives a copy under `~/.claude/skills`". The copy is a copy, not a link. Each skill holds its own `.agents/skills/<skill>/references/`, "Detailed guidance loaded on demand". The page shows no file shared outside a skill directory. "To repair or refresh skills manually, run: `twg skills install`."
- GitHub CLI, `gh skill install`. <https://cli.github.com/manual/gh_skill_install>. Skills land "at either project scope (inside the current git repository) or user scope (in your home directory, available everywhere)". At project scope, several agents "share the `.agents/skills` directory", and a skill bound for one destination from several hosts "is installed there only once". Local skills "are copied (not symlinked) with local-path tracking metadata injected into frontmatter". Source tracking metadata "enables `gh skill update` to detect changes".
- Vercel `skills` CLI. <https://github.com/vercel-labs/skills>. It keeps one canonical copy and links it into each agent's own root, and issue 1025 below names `~/.agents/skills` as the store of a global install. Symlink is the recommended mode, and `--copy` "creates independent copies for each agent", for where symlinks are not supported. Its issue tracker records what the links cost:
  - <https://github.com/vercel-labs/skills/issues/744>: the skill installs to `~/.agents/skills` but no link reaches `~/.claude/skills`, so Claude Code cannot see it.
  - <https://github.com/vercel-labs/skills/issues/1199>: `update` turns copied skills back into links, because the lock file does not record the install mode, and links do not travel across platforms.
  - <https://github.com/vercel-labs/skills/issues/1025>: a request for a command that rebuilds the links, because rerunning the install over the canonical store is unsafe.

What the sources establish:

- Every tool installs a skill as one whole directory, and each directory carries its own `references/`. No tool reads a supporting file from outside the skill directory.
- Tools that serve several agent roots either copy the directory into each root or link each root to one canonical copy. The links fail in ways the copies do not.
- A tool that writes the copies from one embedded source keeps one authored file, so a fix still lands once. That is an inference from the sources above, not a quote.

## Where the sources conflict

| Topic                       | Positions                                                                                                                                                             | The rules take                                                                                         |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Size unit                   | 500 lines (specification, Claude docs, OpenAI skill-creator). 5,000 tokens (specification, Claude overview). 5,000 words (Anthropic guide, OpenAI skill-creator).     | Characters, because the measurement ranks them first and the three units disagree for unwrapped prose. |
| Table of contents threshold | 100 lines (Claude best practices, OpenAI skill-creator). 300 lines (Anthropic skill-creator).                                                                         | 8,000 characters, the stricter end measured on unwrapped prose.                                        |
| Reasons in rules            | Explain the why (Agent Skills, Anthropic skill-creator, Claude prompting). State what to do, not how or why (Claude Code skills).                                     | A reason of one clause, only where it changes behavior.                                                |
| Emphasis                    | A worked example suggests "MUST filter" over "always filter" (Claude skill best practices). Dial back aggressive wording (Claude prompting, Anthropic skill-creator). | No all-caps emphasis. A hard rule becomes a check.                                                     |
| Description voice           | Third person (Claude). Imperative "Use this skill when" (Agent Skills).                                                                                               | "[What it does]. Use when …", which satisfies both.                                                    |

## What the research could not verify

- No primary source prescribes Markdown decision tables. `skill-format:the-structure-follows-the-content` chooses them for a closed choice on its own reasoning.
- No primary source sets a size cap for an individual reference file beyond "focused" and "smaller". The 32,000-character cap follows the Codex 32 KiB limit for instruction files.
- The OpenAI tokens-per-character help article returned HTTP 403 on 2026-10-08 and 2026-10-10, so its rule was confirmed only through secondary quotes.
