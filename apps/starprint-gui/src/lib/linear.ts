import type { TaskCard } from "./api";

const ENDPOINT = "https://api.linear.app/graphql";

/** The fields of a Linear issue a task card can carry. */
export interface Issue {
  id: string;
  /** The key, such as `OPC-123`. */
  identifier: string;
  title: string;
  /** 0 is none, 1 urgent, 2 high, 3 medium, 4 low. */
  priority: number;
  /** An ISO date, or null. */
  dueDate: string | null;
}

/**
 * Runs one query with a personal API key. Linear answers a bad key with
 * a 400 and an `errors` list rather than a 401, so the body is what
 * decides.
 */
async function query<T>(apiKey: string, document: string): Promise<T> {
  const response = await fetch(ENDPOINT, {
    method: "POST",
    headers: { "Content-Type": "application/json", Authorization: apiKey },
    body: JSON.stringify({ query: document }),
  });
  const body = (await response.json()) as {
    data?: T;
    errors?: { message: string }[];
  };
  if (!response.ok || !body.data || body.errors?.length) {
    throw new Error(
      body.errors?.[0]?.message ?? `Linear answered ${response.status}`,
    );
  }
  return body.data;
}

/** The name of whoever the key belongs to. */
export async function whoami(apiKey: string): Promise<string> {
  const { viewer } = await query<{ viewer: { name: string } }>(
    apiKey,
    "{ viewer { name } }",
  );
  return viewer.name;
}

/**
 * Open issues assigned to the key's owner. One page, newest first, so
 * beyond 250 of them it is the oldest that fall off the end.
 */
export async function assignedIssues(apiKey: string): Promise<Issue[]> {
  const { issues } = await query<{ issues: { nodes: Issue[] } }>(
    apiKey,
    `{
      issues(
        first: 250
        orderBy: createdAt
        filter: {
          assignee: { isMe: { eq: true } }
          state: { type: { nin: ["completed", "canceled"] } }
        }
      ) {
        nodes { id identifier title priority dueDate }
      }
    }`,
  );
  return issues.nodes;
}

/** Urgent and high get the banner; medium and low do not. */
export function toCard(issue: Issue): TaskCard {
  return {
    text: issue.title,
    priority: issue.priority === 1 || issue.priority === 2,
    reference: issue.identifier,
    due: issue.dueDate,
  };
}
