/**
 * Sidebar task-list filter menu (UI-SCREEN-23; docs/screens/search-palette.md
 * §Filter menu, docs/features/search-and-filter.md; REQ-SEARCH-04/05; Phase 6
 * slice 5).
 *
 * Captured: the five rows (Type, Status, Last activity, Group by, Sort by)
 * with their option vocabularies; "changes apply immediately"; "multiple
 * filters compose (AND)". Rebuild-defined: the reset affordance appears when
 * a filter is non-default (the shipped reset's location was unknown), the
 * active-filter badge on the trigger, and the "Unread" option renders
 * disabled — the rebuild has no unread tracking, so the capability is stated
 * unavailable rather than faked (docs/14 §9). Pinned-task behavior was never
 * observed and no pin state exists; pinning is not built.
 *
 * The Type options are surface-dependent (docs/features/search-and-filter.md):
 * the sidebar task list is scoped by the Cowork|Code mode switch (Phase 4),
 * so the submenu offers the current surface's vocabulary.
 */
import { useEffect, useRef, useState } from "react";
import { useFocusTrap } from "./focusTrap";

export interface TaskFilters {
  type: "all" | "chat" | "code";
  status: "active" | "archived";
  activity: "all" | "today" | "7d" | "30d";
  groupBy: "none" | "date" | "project" | "status";
  sortBy: "activity" | "created" | "name";
}

export const DEFAULT_FILTERS: TaskFilters = {
  type: "all",
  status: "active",
  activity: "all",
  groupBy: "none",
  sortBy: "activity",
};

const STORAGE_KEY = "tasks.filters";

export function loadFilters(): TaskFilters {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_FILTERS;
    const parsed = JSON.parse(raw) as Partial<TaskFilters>;
    return { ...DEFAULT_FILTERS, ...parsed };
  } catch {
    return DEFAULT_FILTERS;
  }
}

export function saveFilters(filters: TaskFilters): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(filters));
}

export function activeFilterCount(filters: TaskFilters): number {
  let count = 0;
  if (filters.type !== DEFAULT_FILTERS.type) count += 1;
  if (filters.status !== DEFAULT_FILTERS.status) count += 1;
  if (filters.activity !== DEFAULT_FILTERS.activity) count += 1;
  if (filters.groupBy !== DEFAULT_FILTERS.groupBy) count += 1;
  if (filters.sortBy !== DEFAULT_FILTERS.sortBy) count += 1;
  return count;
}

type MenuRow = keyof Pick<TaskFilters, "type" | "status" | "activity" | "groupBy" | "sortBy">;

const ROW_LABELS: Record<MenuRow, string> = {
  type: "Type",
  status: "Status",
  activity: "Last activity",
  groupBy: "Group by",
  sortBy: "Sort by",
};

/** True when the filter hides this timestamp (Last activity window). */
export function withinActivity(filters: TaskFilters, lastActivityAt: number): boolean {
  if (filters.activity === "all") return true;
  const now = new Date();
  if (filters.activity === "today") {
    const midnight = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
    return lastActivityAt >= midnight;
  }
  const days = filters.activity === "7d" ? 7 : 30;
  return lastActivityAt >= now.getTime() - days * 86_400_000;
}

/** The recency group label for Group by = Date (docs/features/search-and-filter.md). */
export function dateGroupLabel(lastActivityAt: number): string {
  const now = new Date();
  const midnight = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  if (lastActivityAt >= midnight) return "Today";
  if (lastActivityAt >= midnight - 86_400_000) return "Yesterday";
  if (lastActivityAt >= midnight - 7 * 86_400_000) return "Previous 7 days";
  return "Older";
}

interface Props {
  filters: TaskFilters;
  onChange: (filters: TaskFilters) => void;
  /** The surface vocabulary the current mode supports (surface-dependent). */
  surface: "chat" | "code";
  onClose: () => void;
}

export function TaskFilterMenu({ filters, onChange, surface, onClose }: Props) {
  const [openRow, setOpenRow] = useState<MenuRow | null>(null);
  const trapRef = useRef<HTMLDivElement>(null);
  useFocusTrap(trapRef, true);

  useEffect(() => {
    const onOutside = (event: MouseEvent) => {
      if (event.target instanceof Node && trapRef.current && !trapRef.current.contains(event.target)) {
        onClose();
      }
    };
    document.addEventListener("mousedown", onOutside);
    return () => document.removeEventListener("mousedown", onOutside);
  }, [onClose]);

  const options = (row: MenuRow): { value: string; label: string; disabled?: boolean }[] => {
    switch (row) {
      case "type":
        return surface === "code"
          ? [
              { value: "all", label: "All types" },
              { value: "code", label: "Code" },
            ]
          : [
              { value: "all", label: "All types" },
              { value: "chat", label: "Cowork" },
            ];
      case "status":
        return [
          { value: "active", label: "Active" },
          { value: "archived", label: "Archived" },
          // No unread tracking exists in the rebuild; the option is stated
          // unavailable instead of silently matching nothing.
          { value: "unread", label: "Unread", disabled: true },
        ];
      case "activity":
        return [
          { value: "all", label: "Any time" },
          { value: "today", label: "Today" },
          { value: "7d", label: "Last 7 days" },
          { value: "30d", label: "Last 30 days" },
        ];
      case "groupBy":
        return [
          { value: "none", label: "None" },
          { value: "date", label: "Date" },
          { value: "project", label: "Project" },
          { value: "status", label: "Status" },
        ];
      case "sortBy":
        return [
          { value: "activity", label: "Last activity" },
          { value: "created", label: "Date created" },
          { value: "name", label: "Name" },
        ];
    }
  };

  const currentLabel = (row: MenuRow): string => {
    const value = filters[row];
    if (row === "type") {
      if (value === "all") return "All";
      return surface === "code" ? "Code" : "Cowork";
    }
    if (row === "activity" && value === "all") return "All";
    const option = options(row).find((candidate) => candidate.value === value);
    return option?.label ?? String(value);
  };

  return (
    <div className="filter-menu" role="menu" aria-label="Filter tasks" ref={trapRef}>
      {(Object.keys(ROW_LABELS) as MenuRow[]).map((row) =>
        openRow === row ? (
          <div key={row} className="filter-options" role="group" aria-label={ROW_LABELS[row]}>
            {options(row).map((option) => (
              <button
                key={option.value}
                type="button"
                role="menuitemradio"
                aria-checked={filters[row] === option.value}
                disabled={option.disabled}
                title={option.disabled ? "Unread state does not exist in this build" : undefined}
                className="filter-option"
                onClick={() => {
                  if (option.disabled) return;
                  onChange({ ...filters, [row]: option.value } as TaskFilters);
                  // Applying closes the whole menu: the change is already
                  // live in the list, and the badge keeps it discoverable.
                  onClose();
                }}
              >
                {option.label}
              </button>
            ))}
          </div>
        ) : (
          <button
            key={row}
            type="button"
            role="menuitem"
            aria-haspopup="true"
            className="filter-row"
            data-filter-row={row}
            onClick={() => setOpenRow(row)}
          >
            <span>{ROW_LABELS[row]}</span>
            <span className="filter-row-value">
              {currentLabel(row)}
              <span className="filter-row-chevron">›</span>
            </span>
          </button>
        ),
      )}
      {activeFilterCount(filters) > 0 ? (
        <button
          type="button"
          role="menuitem"
          className="filter-reset"
          onClick={() => {
            onChange(DEFAULT_FILTERS);
            onClose();
          }}
        >
          Reset filters
        </button>
      ) : null}
    </div>
  );
}
