import { Component, createRef, useRef } from "react"
import type { ChangeEvent, FormEvent, ReactNode } from "react"
import {
  CircleAlert,
  Database,
  File as FileIcon,
  KeyRound,
  Link2,
  LoaderCircle,
  LockKeyhole,
  Pencil,
  RotateCcw,
  Server,
  Upload,
} from "lucide-react"

import { api, isErrorBody } from "@/lib/api"
import {
  actorFromSelectValue,
  actorSelectValue,
  ANONYMOUS_ACTOR_SELECT_VALUE,
} from "@/lib/actor"
import { defaultStr, typeStr } from "@/lib/contract"
import { GraphQLMutationError, insertRow } from "@/lib/graphql-mutations"
import { isFileField, STORAGE_OBJECT, uploadFile } from "@/lib/storage"
import { cn } from "@/lib/utils"
import type { Contract, Field, Persona, Table } from "@/types"

import { FileThumb } from "@/components/file-thumb"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Sheet,
  SheetBody,
  SheetContent,
  SheetDescription,
  SheetFooter,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { Textarea } from "@/components/ui/textarea"

type Row = Record<string, unknown>

interface Props {
  open: boolean
  contract: Contract
  table: Table
  actor: string | null
  personas: Persona[]
  onActorChange: (actor: string | null) => void
  onOpenChange: (open: boolean) => void
  onInserted: () => void
}

interface FieldDraft {
  included: boolean
  raw: string
}

interface ReferenceOptions {
  loading: boolean
  error: string | null
  target: Table
  key: string
  label: string
  rows: Row[]
  capped: boolean
}

interface FieldUpload {
  busy: boolean
  objectId?: string
  fileName?: string
  fileSize?: number
}

interface State {
  drafts: Record<string, FieldDraft>
  references: Record<string, ReferenceOptions>
  uploads: Record<string, FieldUpload>
  fieldErrors: Record<string, string>
  formError: string | null
  submitting: boolean
}

const REFERENCE_LIMIT = 200
const MAX_UPLOAD_BYTES = 25 * 1024 * 1024

/**
 * The metadata-driven insert form. It deliberately keeps "omitted" separate
 * from a value: omission is how defaults apply, and it is how an optional
 * field with no default becomes NULL on insert.
 */
export class InsertRowSheet extends Component<Props, State> {
  state: State = initialState(this.props.table)
  private referenceGeneration = 0
  private uploadGeneration = 0
  private uploadSequences = new Map<string, number>()
  private uploadControllers = new Map<string, AbortController>()
  private bodyRef = createRef<HTMLDivElement>()

  componentDidUpdate(prev: Props) {
    const opened = this.props.open && !prev.open
    const tableChanged = this.props.table.name !== prev.table.name
    if (this.props.open && (opened || tableChanged)) {
      this.beginSession()
      return
    }
    if (this.props.open && this.props.actor !== prev.actor) void this.loadReferences()
  }

  componentWillUnmount() {
    this.referenceGeneration += 1
    this.uploadGeneration += 1
    this.abortAllUploads()
  }

  private beginSession = () => {
    this.referenceGeneration += 1
    this.uploadGeneration += 1
    this.abortAllUploads()
    this.setState(initialState(this.props.table), () => void this.loadReferences())
  }

  private loadReferences = async () => {
    const generation = ++this.referenceGeneration
    const targets = referencedTables(this.props.contract, this.props.table)
    if (targets.length === 0) {
      this.setState({ references: {} })
      return
    }

    const loading = Object.fromEntries(
      targets.map((target) => {
        const key = target.key[0]
        return [
          target.name,
          {
            loading: true,
            error: null,
            target,
            key,
            label: referenceLabelField(target),
            rows: [],
            capped: false,
          } satisfies ReferenceOptions,
        ]
      }),
    )
    this.setState({ references: loading })

    const entries = await Promise.all(
      targets.map(async (target): Promise<[string, ReferenceOptions]> => {
        const key = target.key[0]
        const label = referenceLabelField(target)
        const params = new URLSearchParams({ limit: String(REFERENCE_LIMIT) })
        if (label !== key) params.set("order", `${label}.asc`)
        if (target.name === STORAGE_OBJECT) params.set("state", "eq.committed")
        const res = await api(
          `/rest/v1/${encodeURIComponent(target.name)}?${params}`,
          this.props.actor,
        )
        if (res.status !== 200) {
          const error = isErrorBody(res.body)
            ? (res.body.error.message ?? res.body.error.code)
            : `could not load ${target.name} rows (HTTP ${res.status})`
          return [
            target.name,
            { loading: false, error, target, key, label, rows: [], capped: false },
          ]
        }
        const rows = ((res.body as { rows?: Row[] }).rows ?? []).filter(
          (row) => row[key] !== null && row[key] !== undefined,
        )
        return [
          target.name,
          {
            loading: false,
            error: null,
            target,
            key,
            label,
            rows,
            capped: rows.length >= REFERENCE_LIMIT,
          },
        ]
      }),
    )

    if (generation !== this.referenceGeneration) return
    this.setState({ references: Object.fromEntries(entries) })
  }

  private setIncluded = (field: Field, included: boolean) => {
    const clearFile = !included && isFileField(field)
    if (clearFile) this.invalidateFieldUpload(field.name)
    this.setState((state) => ({
      drafts: {
        ...state.drafts,
        [field.name]: {
          ...state.drafts[field.name],
          included,
          raw: clearFile ? "" : state.drafts[field.name].raw,
        },
      },
      uploads: included ? state.uploads : omitKey(state.uploads, field.name),
      fieldErrors: omitKey(state.fieldErrors, field.name),
      formError: null,
    }))
  }

  private setRaw = (field: Field, raw: string) => {
    if (isFileField(field)) this.invalidateFieldUpload(field.name)
    this.setState((state) => ({
      drafts: {
        ...state.drafts,
        [field.name]: { included: true, raw },
      },
      uploads: isFileField(field) ? omitKey(state.uploads, field.name) : state.uploads,
      fieldErrors: omitKey(state.fieldErrors, field.name),
      formError: null,
    }))
  }

  private changeActor = (actor: string | null) => {
    if (actor === this.props.actor) return
    const uploadedFields = new Set(Object.keys(this.state.uploads))
    const actorFields = new Set(
      this.props.table.fields
        .filter((field) => field.default?.kind === "actor")
        .map((field) => field.name),
    )
    this.uploadGeneration += 1
    this.abortAllUploads()
    this.setState(
      (state) => ({
        drafts: Object.fromEntries(
          this.props.table.fields.map((field) => [
            field.name,
            uploadedFields.has(field.name) ? draftFor(field) : state.drafts[field.name],
          ]),
        ),
        uploads: {},
        fieldErrors: Object.fromEntries(
          Object.entries(state.fieldErrors).filter(
            ([field]) => !uploadedFields.has(field) && !actorFields.has(field),
          ),
        ),
        formError:
          uploadedFields.size > 0
            ? "Uploaded file selections were cleared because the Actor changed."
            : null,
      }),
      () => this.props.onActorChange(actor),
    )
  }

  private uploadField = async (field: Field, file: File) => {
    if (file.size > MAX_UPLOAD_BYTES) {
      this.setState(
        (state) => ({
          fieldErrors: {
            ...state.fieldErrors,
            [field.name]: `Choose a file smaller than ${formatBytes(MAX_UPLOAD_BYTES)}.`,
          },
        }),
        this.revealErrors,
      )
      return
    }

    const previousDraft = this.state.drafts[field.name] ?? draftFor(field)
    const previousUpload = this.state.uploads[field.name]
    const generation = this.uploadGeneration
    const { sequence, controller } = this.beginFieldUpload(field.name)
    this.setState((state) => ({
      uploads: {
        ...state.uploads,
        [field.name]: { busy: true, fileName: file.name, fileSize: file.size },
      },
      fieldErrors: omitKey(state.fieldErrors, field.name),
      formError: null,
    }))

    try {
      const objectId = await uploadFile(file, this.props.actor, controller.signal)
      if (!this.isCurrentUpload(field.name, generation, sequence)) return
      this.uploadControllers.delete(field.name)
      this.setState((state) => ({
        drafts: {
          ...state.drafts,
          [field.name]: { included: true, raw: encodeReferenceValue(objectId) },
        },
        uploads: {
          ...state.uploads,
          [field.name]: {
            busy: false,
            objectId,
            fileName: file.name,
            fileSize: file.size,
          },
        },
      }))
    } catch (error) {
      if (!this.isCurrentUpload(field.name, generation, sequence)) return
      this.uploadControllers.delete(field.name)
      const message = error instanceof Error ? error.message : String(error)
      this.setState(
        (state) => ({
          drafts: { ...state.drafts, [field.name]: previousDraft },
          uploads: previousUpload
            ? { ...state.uploads, [field.name]: previousUpload }
            : omitKey(state.uploads, field.name),
          fieldErrors: { ...state.fieldErrors, [field.name]: message },
        }),
        this.revealErrors,
      )
    }
  }

  private beginFieldUpload = (field: string): { sequence: number; controller: AbortController } => {
    this.uploadControllers.get(field)?.abort()
    const controller = new AbortController()
    this.uploadControllers.set(field, controller)
    const sequence = (this.uploadSequences.get(field) ?? 0) + 1
    this.uploadSequences.set(field, sequence)
    return { sequence, controller }
  }

  private invalidateFieldUpload = (field: string) => {
    this.uploadControllers.get(field)?.abort()
    this.uploadControllers.delete(field)
    this.uploadSequences.set(field, (this.uploadSequences.get(field) ?? 0) + 1)
  }

  private abortAllUploads = () => {
    for (const controller of this.uploadControllers.values()) controller.abort()
    this.uploadControllers.clear()
  }

  private isCurrentUpload = (field: string, generation: number, sequence: number): boolean =>
    generation === this.uploadGeneration && sequence === this.uploadSequences.get(field)

  private revealErrors = () => {
    const body = this.bodyRef.current
    if (!body) return
    const invalid = body.querySelector<HTMLElement>('[aria-invalid="true"]')
    if (invalid) {
      invalid.scrollIntoView({ block: "center" })
      invalid.focus()
      return
    }
    body.scrollTo({ top: 0, behavior: "smooth" })
  }

  private close = () => {
    if (this.state.submitting) return
    this.dismiss()
  }

  private dismiss = () => {
    this.referenceGeneration += 1
    this.uploadGeneration += 1
    this.abortAllUploads()
    this.props.onOpenChange(false)
  }

  private submit = async (event: FormEvent) => {
    event.preventDefault()
    if (this.state.submitting || Object.values(this.state.uploads).some((upload) => upload.busy)) {
      return
    }

    const missingActor = this.props.table.fields.filter(
      (field) =>
        field.default?.kind === "actor" && !field.optional && this.props.actor === null,
    )
    if (missingActor.length > 0) {
      this.setState(
        {
          fieldErrors: Object.fromEntries(
            missingActor.map((field) => [field.name, "Select an Actor before inserting."]),
          ),
          formError: "This row requires an Actor.",
        },
        this.revealErrors,
      )
      return
    }

    const parsed = buildInsertObject(this.props.table, this.state.drafts)
    if (Object.keys(parsed.errors).length > 0) {
      this.setState(
        { fieldErrors: parsed.errors, formError: "Check the highlighted fields." },
        this.revealErrors,
      )
      return
    }

    this.setState({ submitting: true, formError: null, fieldErrors: {} })
    try {
      await insertRow(this.props.table, parsed.object, this.props.actor)
      this.props.onInserted()
      this.setState({ submitting: false }, this.dismiss)
    } catch (error) {
      if (error instanceof GraphQLMutationError) {
        const fieldErrors = Object.fromEntries(
          error.fields.map((field) => [field, error.message]),
        )
        this.setState(
          {
            submitting: false,
            fieldErrors,
            formError:
              error.code === "graphql_error" ? error.message : `${error.code} · ${error.message}`,
          },
          this.revealErrors,
        )
      } else {
        this.setState(
          {
            submitting: false,
            formError: error instanceof Error ? error.message : String(error),
          },
          this.revealErrors,
        )
      }
    }
  }

  render() {
    const { open, table, actor } = this.props
    const required = table.fields.filter((field) => !field.optional && field.default == null)
    const defaulted = table.fields.filter((field) => field.default != null)
    const optional = table.fields.filter((field) => field.optional && field.default == null)
    const { formError, submitting } = this.state
    const uploading = Object.values(this.state.uploads).some((upload) => upload.busy)
    const missingActor = table.fields.some(
      (field) => field.default?.kind === "actor" && !field.optional && actor === null,
    )

    return (
      <Sheet open={open} onOpenChange={(next) => (next ? this.props.onOpenChange(true) : this.close())}>
        <SheetContent className="sm:w-[min(78vw,760px)] sm:max-w-[760px]">
          <form
            onSubmit={this.submit}
            className="flex min-h-0 flex-1 flex-col"
            aria-busy={submitting}
          >
            <SheetHeader className="px-6 py-5">
              <SheetTitle className="flex items-center gap-2 text-base">
                Add new row to <code className="text-[13px] font-semibold">{table.name}</code>
              </SheetTitle>
              <SheetDescription>
                The form follows the compiled contract. Unset fields are omitted so server defaults
                and nullable semantics stay authoritative.
              </SheetDescription>
            </SheetHeader>

            <SheetBody ref={this.bodyRef} className="px-6 py-0">
              {formError ? <ErrorNotice>{formError}</ErrorNotice> : null}

              <fieldset disabled={submitting} className="m-0 min-w-0 border-0 p-0">
                {required.length ? (
                  <FieldSection
                    title="Required fields"
                    description="These fields have no default and must be included."
                  >
                    {required.map((field) => this.renderField(field, actor))}
                  </FieldSection>
                ) : null}

                {defaulted.length ? (
                  <FieldSection
                    title="Defaulted fields"
                    description="Leave these unset to let the server apply the compiled default."
                  >
                    {defaulted.map((field) => this.renderField(field, actor))}
                  </FieldSection>
                ) : null}

                {optional.length ? (
                  <FieldSection
                    title="Optional fields"
                    description="Omitted fields store NULL. Set one only when the row needs a value."
                  >
                    {optional.map((field) => this.renderField(field, actor))}
                  </FieldSection>
                ) : null}
              </fieldset>
            </SheetBody>

            <SheetFooter className="px-6 py-3.5">
              <div className="flex items-center gap-2">
                <Button type="button" variant="outline" size="lg" onClick={this.close} disabled={submitting}>
                  Cancel
                </Button>
                <Button type="submit" size="lg" disabled={submitting || uploading || missingActor}>
                  {submitting || uploading ? <LoaderCircle className="animate-spin" /> : <Database />}
                  {submitting ? "Inserting…" : uploading ? "Uploading…" : "Insert row"}
                </Button>
              </div>
            </SheetFooter>
          </form>
        </SheetContent>
      </Sheet>
    )
  }

  private renderField(field: Field, actor: string | null): ReactNode {
    const draft = this.state.drafts[field.name] ?? draftFor(field)
    const error = this.state.fieldErrors[field.name]
    const actorStamped = field.default?.kind === "actor"
    const canOmit = field.optional || field.default != null
    const reference = field.type.kind === "ref" ? this.state.references[field.type.table ?? ""] : undefined

    return (
      <div
        key={field.name}
        className="grid gap-3 border-b py-5 last:border-b-0 sm:grid-cols-[190px_minmax(0,1fr)] sm:gap-6"
      >
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-1.5">
            {this.props.table.key.includes(field.name) ? (
              <KeyRound size={12} className="text-muted-foreground" />
            ) : null}
            <span className="font-mono text-[13px] font-semibold">{field.name}</span>
          </div>
          <div className="mt-1 flex flex-wrap items-center gap-1.5">
            <span className="font-mono text-[11px] text-muted-foreground">{typeStr(field.type)}</span>
            <FieldBadge field={field} />
          </div>
          {field.doc ? <p className="mt-2 text-[11px] leading-relaxed text-muted-foreground">{field.doc}</p> : null}
        </div>

        <div className="min-w-0">
          {actorStamped ? (
            <LockedActorField
              field={field.name}
              actor={actor}
              optional={Boolean(field.optional)}
              personas={this.props.personas}
              onActorChange={this.changeActor}
            />
          ) : draft.included ? (
            <div className="flex items-start gap-2">
              <div className="min-w-0 flex-1">
                <FieldControl
                  field={field}
                  label={field.name}
                  value={draft.raw}
                  reference={reference}
                  upload={this.state.uploads[field.name]}
                  invalid={Boolean(error)}
                  onChange={(raw) => this.setRaw(field, raw)}
                  onUpload={(file) => void this.uploadField(field, file)}
                />
              </div>
              {canOmit ? (
                <Button
                  type="button"
                  variant="outline"
                  size="icon-lg"
                  title={field.default ? "Use server default" : "Omit field"}
                  onClick={() => this.setIncluded(field, false)}
                >
                  <RotateCcw />
                </Button>
              ) : null}
            </div>
          ) : (
            <button
              type="button"
              onClick={() => this.setIncluded(field, true)}
              className="flex min-h-9 w-full items-center justify-between gap-3 rounded-md border bg-muted/30 px-3 py-2 text-left transition-colors hover:bg-muted/60"
            >
              <span className="truncate text-xs text-muted-foreground">
                {field.default ? `Server default ${defaultStr(field.default)}` : "NULL · field omitted"}
              </span>
              <span className="flex shrink-0 items-center gap-1 text-[11px] font-medium text-foreground">
                <Pencil size={12} /> Set value
              </span>
            </button>
          )}

          {field.default && !actorStamped ? (
            <p className="mt-1.5 text-[11px] text-muted-foreground">Default {defaultStr(field.default)}</p>
          ) : null}
          {field.type.kind === "ref" ? (
            <ReferenceHint options={reference} />
          ) : null}
          {field.check ? (
            <p className="mt-1.5 text-[11px] text-muted-foreground">
              Validated by <code>{field.check}</code>
            </p>
          ) : null}
          {error ? <p className="mt-1.5 text-[11px] text-destructive">{error}</p> : null}
        </div>
      </div>
    )
  }
}

function FieldSection({
  title,
  description,
  children,
}: {
  title: string
  description: string
  children: ReactNode
}) {
  return (
    <section>
      <div className="-mx-6 border-y bg-muted/30 px-6 py-4">
        <h3 className="text-sm font-semibold">{title}</h3>
        <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>
      </div>
      {children}
    </section>
  )
}

function FieldBadge({ field }: { field: Field }) {
  return (
    <>
      {!field.optional && field.default == null ? <Badge variant="outline">required</Badge> : null}
      {field.optional ? <Badge variant="outline">nullable</Badge> : null}
      {field.default?.kind === "actor" ? (
        <Badge variant="outline">server</Badge>
      ) : field.default ? (
        <Badge variant="outline">default</Badge>
      ) : null}
    </>
  )
}

function LockedActorField({
  field,
  actor,
  optional,
  personas,
  onActorChange,
}: {
  field: string
  actor: string | null
  optional: boolean
  personas: Persona[]
  onActorChange: (actor: string | null) => void
}) {
  return (
    <div>
      <Select
        value={actor === null && !optional ? null : actorSelectValue(actor)}
        onValueChange={(value) => onActorChange(actorFromSelectValue(value))}
      >
        <SelectTrigger
          className="h-9 w-full bg-muted/40 px-3"
          aria-label={`${field} Actor`}
          aria-invalid={!optional && actor === null}
        >
          <LockKeyhole size={14} className="text-muted-foreground" />
          <SelectValue placeholder="Select an Actor">
            {(value: unknown) => {
              const selectedActor = actorFromSelectValue(value)
              return selectedActor === null
                ? optional
                  ? "Anonymous · server will store NULL"
                  : "Select an Actor before inserting"
                : `Actor · ${personas.find((persona) => persona.actor === selectedActor)?.label ?? selectedActor}`
            }}
          </SelectValue>
        </SelectTrigger>
        <SelectContent alignItemWithTrigger={false} className="min-w-[300px]">
          {optional ? (
            <SelectItem value={ANONYMOUS_ACTOR_SELECT_VALUE}>anonymous · store NULL</SelectItem>
          ) : null}
          {personas.map((persona) => (
            <SelectItem key={persona.actor} value={actorSelectValue(persona.actor)}>
              <span>{persona.label}</span>
              <span className="max-w-52 truncate font-mono text-[10px] text-muted-foreground">
                {persona.actor}
              </span>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <p className="mt-1.5 flex items-center gap-1 text-[11px] text-muted-foreground">
        <Server size={11} /> <code>= me</code> uses Studio's Actor request context; it is never sent as a field value.
      </p>
    </div>
  )
}

function FieldControl({
  field,
  label,
  value,
  reference,
  upload,
  invalid,
  onChange,
  onUpload,
}: {
  field: Field
  label: string
  value: string
  reference?: ReferenceOptions
  upload?: FieldUpload
  invalid: boolean
  onChange: (value: string) => void
  onUpload: (file: File) => void
}) {
  if (!isKnownFieldKind(field.type.kind)) {
    return (
      <div
        className="flex min-h-9 items-center rounded-md border border-destructive/30 bg-destructive/5 px-3 text-xs text-destructive"
        role="alert"
      >
        Unsupported contract type <code>{field.type.kind}</code>
      </div>
    )
  }

  if (field.type.kind === "ref") {
    if (isFileField(field)) {
      return (
        <StorageObjectControl
          value={value}
          reference={reference}
          upload={upload}
          invalid={invalid}
          label={label}
          onChange={onChange}
          onUpload={onUpload}
        />
      )
    }
    return (
      <ReferenceSelect
        value={value}
        reference={reference}
        invalid={invalid}
        label={label}
        placeholder={`Select ${field.type.table ?? "row"}`}
        onChange={onChange}
      />
    )
  }

  if (field.type.kind === "set") {
    return (
      <Select value={value || null} onValueChange={(next) => onChange(String(next))}>
        <SelectTrigger className="h-9 w-full px-3" aria-label={label} aria-invalid={invalid}>
          <SelectValue placeholder="Select a value" />
        </SelectTrigger>
        <SelectContent>
          {(field.type.values ?? []).map((option) => (
            <SelectItem key={option} value={option}>
              <span className="font-mono">{option}</span>
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    )
  }

  if (field.type.kind === "bool") {
    return (
      <Select value={value || null} onValueChange={(next) => onChange(String(next))}>
        <SelectTrigger className="h-9 w-full px-3" aria-label={label} aria-invalid={invalid}>
          <SelectValue placeholder="Select true or false" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem value="true">TRUE</SelectItem>
          <SelectItem value="false">FALSE</SelectItem>
        </SelectContent>
      </Select>
    )
  }

  if (field.type.kind === "text") {
    return (
      <Textarea
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder="Enter text"
        aria-label={label}
        aria-invalid={invalid}
      />
    )
  }

  const type = field.type.kind === "int" || field.type.kind === "float" ? "number" : field.type.kind === "timestamp" ? "datetime-local" : "text"
  return (
    <Input
      type={type}
      step={field.type.kind === "int" ? 1 : field.type.kind === "float" ? "any" : undefined}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      placeholder={field.type.kind === "uuid" ? "00000000-0000-0000-0000-000000000000" : "Enter value"}
      aria-label={label}
      aria-invalid={invalid}
      className="h-9 px-3"
    />
  )
}

function StorageObjectControl({
  value,
  reference,
  upload,
  invalid,
  label,
  onChange,
  onUpload,
}: {
  value: string
  reference?: ReferenceOptions
  upload?: FieldUpload
  invalid: boolean
  label: string
  onChange: (value: string) => void
  onUpload: (file: File) => void
}) {
  const inputRef = useRef<HTMLInputElement>(null)
  const uploaded = upload?.objectId

  const pick = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    event.target.value = ""
    if (file) onUpload(file)
  }

  return (
    <div className="space-y-3">
      <input
        ref={inputRef}
        type="file"
        hidden
        disabled={upload?.busy}
        aria-label={`Upload ${label}`}
        onChange={pick}
      />
      <div
        className={cn(
          "flex min-h-16 items-center gap-3 rounded-md border bg-muted/20 p-3",
          invalid && "border-destructive/50",
        )}
      >
        {uploaded ? (
          <FileThumb id={uploaded} size={38} className="shrink-0" />
        ) : (
          <span className="flex size-9 shrink-0 items-center justify-center rounded-md border bg-background text-muted-foreground">
            {upload?.busy ? <LoaderCircle size={16} className="animate-spin" /> : <FileIcon size={16} />}
          </span>
        )}
        <div className="min-w-0 flex-1">
          <p className="truncate text-xs font-medium">
            {upload?.fileName ?? "Upload a new file"}
          </p>
          <p className="mt-0.5 truncate text-[11px] text-muted-foreground">
            {upload?.busy
              ? "Uploading through the storage gate…"
              : uploaded
                ? `${upload.fileSize === undefined ? "Uploaded" : formatBytes(upload.fileSize)} · attached when this row is inserted`
                : `Any file up to ${formatBytes(MAX_UPLOAD_BYTES)}`}
          </p>
        </div>
        <Button
          type="button"
          variant="outline"
          size="lg"
          disabled={upload?.busy}
          aria-invalid={invalid}
          onClick={() => inputRef.current?.click()}
        >
          {upload?.busy ? <LoaderCircle className="animate-spin" /> : <Upload />}
          {upload?.busy ? "Uploading…" : uploaded ? "Replace" : "Upload file"}
        </Button>
      </div>

      <div className="flex items-center gap-2 text-[10px] uppercase tracking-wide text-muted-foreground">
        <span className="h-px flex-1 bg-border" />
        or choose an existing object
        <span className="h-px flex-1 bg-border" />
      </div>
      <ReferenceSelect
        value={uploaded ? "" : value}
        reference={reference}
        invalid={invalid}
        label={`${label}: existing storage object`}
        placeholder="Select an uploaded file"
        onChange={onChange}
      />
    </div>
  )
}

function ReferenceSelect({
  value,
  reference,
  invalid,
  label,
  placeholder,
  onChange,
}: {
  value: string
  reference?: ReferenceOptions
  invalid: boolean
  label: string
  placeholder: string
  onChange: (value: string) => void
}) {
  const emptyLabel = reference?.loading
    ? "Loading referenced rows…"
    : reference?.error
      ? "Referenced rows unavailable"
      : placeholder

  return (
    <Select value={value || null} onValueChange={(next) => onChange(String(next))}>
      <SelectTrigger
        className="h-9 w-full px-3"
        aria-label={label}
        aria-invalid={invalid}
      >
        <SelectValue placeholder={emptyLabel}>
          {(selected: unknown) =>
            selected == null || selected === ""
              ? emptyLabel
              : selectedReferenceLabel(String(selected), reference)
          }
        </SelectValue>
      </SelectTrigger>
      <SelectContent alignItemWithTrigger={false} className="min-w-[340px] max-w-[560px]">
        {(reference?.rows ?? []).map((row) => {
          const token = encodeReferenceValue(row[reference!.key])
          return (
            <SelectItem key={token} value={token}>
              <span className="min-w-0 flex-1 truncate">{referenceLabel(row, reference!)}</span>
              {reference!.label !== reference!.key ? (
                <span className="max-w-48 truncate font-mono text-[10px] text-muted-foreground">
                  {String(row[reference!.key])}
                </span>
              ) : null}
            </SelectItem>
          )
        })}
      </SelectContent>
    </Select>
  )
}

function ReferenceHint({ options }: { options?: ReferenceOptions }) {
  if (!options) return null
  return (
    <p
      className={cn(
        "mt-1.5 flex items-center gap-1 text-[11px] text-muted-foreground",
        options.error && "text-destructive",
      )}
    >
      <Link2 size={11} />
      {options.error ? (
        options.error
      ) : options.loading ? (
        `Loading ${options.target.name} rows…`
      ) : (
        <>
          References <code>{options.target.name}.{options.key}</code>
          {options.label !== options.key ? <> · labeled by <code>{options.label}</code></> : null}
          {options.capped ? ` · first ${REFERENCE_LIMIT} rows` : ""}
        </>
      )}
    </p>
  )
}

function ErrorNotice({ children }: { children: ReactNode }) {
  return (
    <div
      role="alert"
      className="mt-4 flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2.5 text-xs text-destructive"
    >
      <CircleAlert size={14} className="mt-0.5 shrink-0" />
      <span>{children}</span>
    </div>
  )
}

function initialState(table: Table): State {
  return {
    drafts: Object.fromEntries(table.fields.map((field) => [field.name, draftFor(field)])),
    references: {},
    uploads: {},
    fieldErrors: {},
    formError: null,
    submitting: false,
  }
}

function draftFor(field: Field): FieldDraft {
  return {
    included: !field.optional && field.default == null,
    raw: defaultRaw(field),
  }
}

function defaultRaw(field: Field): string {
  const value = field.default?.value
  return value === undefined || value === null ? "" : String(value)
}

function referencedTables(contract: Contract, table: Table): Table[] {
  const names = new Set(
    table.fields
      .filter((field) => field.type.kind === "ref" && field.default?.kind !== "actor")
      .map((field) => (field.type as { table?: string }).table)
      .filter((name): name is string => Boolean(name)),
  )
  return [...names]
    .map((name) => contract.tables.find((candidate) => candidate.name === name))
    .filter((target): target is Table => Boolean(target?.key[0]))
}

/** Generic contracts do not yet declare a display column for references. */
function referenceLabelField(table: Table): string {
  return (
    table.fields.find(
      (field) => field.name !== table.key[0] && field.type.kind === "text" && field.unique,
    )?.name ??
    table.fields.find((field) => field.name !== table.key[0] && field.type.kind === "text")?.name ??
    table.key[0]
  )
}

function referenceLabel(row: Row, options: ReferenceOptions): string {
  const label = row[options.label]
  return label === null || label === undefined || label === "" ? String(row[options.key]) : String(label)
}

function selectedReferenceLabel(token: string, options?: ReferenceOptions): string {
  const row = options?.rows.find((candidate) => encodeReferenceValue(candidate[options.key]) === token)
  if (row && options) return referenceLabel(row, options)
  try {
    return String(JSON.parse(token) as unknown)
  } catch {
    return token
  }
}

function encodeReferenceValue(value: unknown): string {
  return JSON.stringify(value)
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KiB`
  return `${Math.round((bytes / (1024 * 1024)) * 10) / 10} MiB`
}

function buildInsertObject(
  table: Table,
  drafts: Record<string, FieldDraft>,
): { object: Record<string, unknown>; errors: Record<string, string> } {
  const object: Record<string, unknown> = {}
  const errors: Record<string, string> = {}

  for (const field of table.fields) {
    if (field.default?.kind === "actor") continue
    const draft = drafts[field.name] ?? draftFor(field)
    if (!draft.included) continue
    try {
      object[field.name] = parseFieldValue(field, draft.raw)
    } catch (error) {
      errors[field.name] = error instanceof Error ? error.message : String(error)
    }
  }
  return { object, errors }
}

function parseFieldValue(field: Field, raw: string): unknown {
  switch (field.type.kind) {
    case "int": {
      if (raw.trim() === "") throw new Error("Enter an integer.")
      const value = Number(raw)
      if (!Number.isSafeInteger(value)) throw new Error("Enter a valid integer.")
      return value
    }
    case "float": {
      if (raw.trim() === "") throw new Error("Enter a number.")
      const value = Number(raw)
      if (!Number.isFinite(value)) throw new Error("Enter a valid number.")
      return value
    }
    case "bool":
      if (raw !== "true" && raw !== "false") throw new Error("Select true or false.")
      return raw === "true"
    case "timestamp": {
      if (raw.trim() === "") throw new Error("Choose a date and time.")
      const value = new Date(raw)
      if (Number.isNaN(value.getTime())) throw new Error("Choose a valid date and time.")
      return value.toISOString()
    }
    case "ref":
      if (!raw)
        throw new Error(
          `Select a ${(field.type as { table?: string }).table ?? "referenced row"}.`,
        )
      return JSON.parse(raw) as unknown
    case "set":
      if (!(field.type.values ?? []).includes(raw)) throw new Error("Select an allowed value.")
      return raw
    case "uuid":
      if (raw.trim() === "") throw new Error("Enter a UUID.")
      return raw.trim()
    case "text":
      return raw
    default:
      throw new Error(`Studio does not support the contract type ${field.type.kind}.`)
  }
}

function isKnownFieldKind(kind: string): boolean {
  return ["text", "int", "float", "bool", "timestamp", "uuid", "ref", "set"].includes(kind)
}

function omitKey<T>(record: Record<string, T>, key: string): Record<string, T> {
  const { [key]: _discarded, ...rest } = record
  return rest
}
