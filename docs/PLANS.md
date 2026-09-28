# Execution plans

An execution plan is a living document. Work needs one when it spans sessions, crosses modules or repositories, or carries significant unknowns.
Other work keeps a short plan in the conversation, the commit message, or the pull request description.

## Location

Active plans live in `docs/exec-plans/active/`, one file per body of work, named `YYYY-MM-DD-topic.md`.
Completed plans live in `docs/exec-plans/completed/` until the repository owner chooses to delete them.
Known debt lives in `docs/exec-plans/tech-debt-tracker.md`.
Plans are committed with the work they describe, so another person or agent can resume the work from the repository alone.
Work that spans repositories keeps one plan in the primary repository. That plan records the sequence, the contracts between repositories, progress, and validation.

## Resume from the plan

A fresh agent can continue the work from the plan and the files it names, without the conversation that produced it.
Link `ARCHITECTURE.md`, specifications, and other owning documents instead of copying them.
Record what exists only in the conversation (requirements the user stated, decisions, observations, and the output that proves a result).
Describe the repository and leave out the machine that ran the work (installed tools and skills, their versions, local failures, absolute paths, and uncommitted changes). The next agent can resume the plan in a different clone.
Name only files that Git tracks or that tracked files create or read. Name each by its repository-relative path. Give each command with its working directory.
Name a skill only when the repository tracks its `SKILL.md`.
Define each term that the linked documents do not define.

## Sections

Each plan starts with its title and a link to `docs/PLANS.md`, followed by these sections in this order. A plan that needs more, such as audit findings, adds sections after them.

| Section | Contents |
| --- | --- |
| Purpose | What someone can do after the change, and how to see it working. |
| Progress | Checkboxes. At each stopping point, split a partially complete item into done and remaining. |
| Surprises & Discoveries | Unexpected behavior, each with short evidence such as test output. |
| Decision Log | Each decision, its reason, and its date. |
| Outcomes & Retrospective | What was achieved, what remains, and what was learned, at each milestone and at completion. |
| Context and Orientation | The files, modules, and documents involved, and how they fit together. |
| Plan of Work | The edits in order, each naming its file and change, and the interfaces that must exist when the work is done. |
| Validation and Acceptance | Commands with their working directory and expected output, and the observable behavior that proves success. |
| Idempotence and Recovery | Which steps can be repeated safely, and how to retry or roll back a step that can fail partway or destroy data. |

Progress, Surprises & Discoveries, Decision Log, and Outcomes & Retrospective change as the work proceeds. The other sections change when the approach changes.
Phrase acceptance as behavior a person can check, such as a test that fails before the change and passes after it, a request and its response, or a command transcript.
Scale each section to the work. A section with nothing to record says so in one line.

## Milestones

Split long work into milestones. Each milestone leaves the repository in a state that its own validation can check.
Use a prototype milestone to test an unknown before the plan depends on it. Label it as a prototype, give the command that exercises it, and state the result that keeps or discards it.

## Maintain the plan

Update Progress at every stopping point.
When the approach changes, record the reason in the Decision Log and update every section the change affects.
Continue to the next milestone without asking for approval that the task already gave.
If a missing choice blocks the work, ask that question and continue independent work.
A task changes only its own plan and records problems it finds in other plans in the tech-debt tracker. Only an audit changes other plans.

## Complete the plan

When the last milestone passes validation, complete the plan in the same commit or pull request as that milestone. If the work is abandoned before then, complete the plan with Outcomes & Retrospective stating why.

1. Write Outcomes & Retrospective.
2. Move lasting facts to their owners. Facts useful to future work go in `ARCHITECTURE.md`, `docs/developers/`, or the user-facing page in `website/docs/` that owns them. A decision still in effect goes there with its date and the plan's file name. Rules that always apply go in `AGENTS.md` or a document it links. Rules a tool could enforce go in the tech-debt tracker until a test or lint enforces them.
3. Record remaining debt in `docs/exec-plans/tech-debt-tracker.md`. Each entry names its finding, its evidence, and the condition for removing it.
4. Move the plan to `docs/exec-plans/completed/`.

When completed work turns out incomplete, the task that notices it records it in the tech-debt tracker. The fix is new work with its own plan, which links the completed plan (through Git history if the plan was deleted), states what failed, and adds a check for the case the earlier validation missed. The completed plan stays in `completed/`.
