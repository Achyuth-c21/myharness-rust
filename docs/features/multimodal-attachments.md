# Multimodal Attachments

**Features:** `FEAT-CHAT-03`, `FEAT-COWORK-09`, `FEAT-CODE-09` (related)
**Primary docs:** [12 — AI Architecture](../12-ai-architecture.md), [08](../08-interaction-catalog.md)

## Purpose

Attachments let a task read images, PDFs, text, and documents; generated skills can produce
docx/pptx/xlsx/pdf files. The selected model/provider determines which input modalities are
supported.

## Entry points

- Composer `+` → **Add files or photos**.
- `Ctrl+U`.
- Drag/drop (supported or strongly inferred; visual state unverified).
- Project/folder file context (not the same as an attachment).
- Tool results (tool-output images/files).

The native file picker runs in main; the renderer receives paths/content ids, not raw filesystem
handles.

## Attachment types

| Type | Handling |
|---|---|
| Images (PNG/JPEG/WebP/GIF) | Inline image content or provider file upload; dimensions/size limits |
| PDFs | Native document content or text extraction via `pdf-reading` |
| Plain text/Markdown/CSV/JSON | Inline text/file part |
| Office docs | Extract via `docx`/`pptx`/`xlsx` skills or provider document support |
| Audio | Dictation/input only if provider/platform supports; gated |
| Video | Only if provider supports; not observed |
| Arbitrary files | Referenced in workspace; model may read via tools, not necessarily inline |

The UI should show type, size, and a remove action. Multiple files are supported.

## Capability negotiation

Before send:

1. Resolve the selected model’s input modalities.
2. Validate each attachment (type/size/count).
3. Use native content parts where supported.
4. If unsupported, show a clear error and let the user remove/convert; **do not drop silently**.
5. Record whether a PDF was sent natively or extracted to text.

Provider limits are model-specific. Do not hard-code a single universal limit; read from the
capability descriptor or provider error, and surface it before send where possible.

## Tool results

Tool results can contain text, images, files, or structured content. Requirements:

- Preserve tool-result images/documents to the provider.
- Keep ordering and media types.
- Enforce size limits.
- Do not flatten image tool results to text-only.
- Do not include tool-result content in telemetry.

The current local adapter drops image tool results and flattens text — a known noncompliance.

## Generated documents

Bundled skills can create:

- Word documents (`docx`);
- PowerPoint (`pptx`);
- Excel (`xlsx`);
- PDFs (`pdf`).

These are files/artifacts, not a model output modality. They should be downloadable/previewable in an
isolated viewer, with filenames sanitized and paths confined to the session outputs folder.

## Privacy

- Attachments may contain personal/confidential data. They are sent only to the selected provider as
  part of the request.
- Never send attachment content to telemetry, crash reports, or nonessential services.
- Previews/favicons/iframes are nonessential services and can be blocked by policy.
- Store attachments under the session/account root with restricted permissions.
- Delete with the session/retention rules; handle orphaned temp files.

## States

| State | Behavior |
|---|---|
| No attachment | Placeholder |
| Picker open | Native dialog; form state unchanged |
| Uploading | Progress/chip; send may be disabled |
| Ready | Chip/thumbnail + remove |
| Too large/unsupported | Inline error; no send |
| Provider rejects | Error names the file/limit; draft retained |
| Tool image | Displayed in tool block and preserved to provider |
| Deleted session | Attachments removed/orphan-GCed |

## Requirements

| ID | Requirement |
|---|---|
| `REQ-MM-01` | Validate attachments against the selected model, not a hard-coded global list. |
| `REQ-MM-02` | Never silently drop unsupported content; show which part failed. |
| `REQ-MM-03` | Preserve tool-result images/files and media types. |
| `REQ-MM-04` | PDFs/documents use a documented native or extraction path. |
| `REQ-MM-05` | Attachments are stored privately, deleted with the session, and excluded from telemetry. |
| `REQ-MM-06` | Generated documents are isolated from the app origin. |

**Tests:** `TEST-AI-012`, `TEST-AI-013`, `TEST-SESSION-004`, `TEST-SEC-002`, `TEST-ARCH-006`.
