import { useState } from "react";
import { format } from "date-fns";
import {
  DEFAULT_NOTE,
  DEFAULT_PICTURE,
  DEFAULT_QR,
  DEFAULT_TEST_PAGE,
  DEFAULT_TEXT,
  type Job,
} from "@/lib/api";
import { isReady, type Jobs, type Kind } from "@/lib/job";

/** What each form opens with; a task card is due the day it is written. */
function fresh(): Jobs {
  return {
    "task-card": {
      kind: "task-card",
      text: "",
      priority: false,
      reference: null,
      due: format(new Date(), "yyyy-MM-dd"),
    },
    text: { kind: "text", ...DEFAULT_TEXT },
    note: { kind: "note", ...DEFAULT_NOTE },
    qr: { kind: "qr", ...DEFAULT_QR },
    "test-page": { kind: "test-page", ...DEFAULT_TEST_PAGE },
    picture: { kind: "picture", ...DEFAULT_PICTURE },
  };
}

/**
 * The forms' state: a job of every kind, so each tab keeps what was
 * typed into it, the picture chosen, and which of them is showing.
 */
export function useJobForm() {
  const [workflow, setWorkflow] = useState<Kind>("task-card");
  const [jobs, setJobs] = useState(fresh);
  const [file, setFile] = useState<File | null>(null);

  const job = jobs[workflow];
  // Only a picture job carries the picture.
  const image = job.kind === "picture" ? file : null;

  /** Keeps `job` as what its kind's form holds. */
  const put = (job: Job) => setJobs((jobs) => ({ ...jobs, [job.kind]: job }));

  /** Puts `job` in its form and shows it, to change and send again. */
  const load = (job: Job) => {
    put(job);
    setWorkflow(job.kind);
  };

  return {
    workflow,
    setWorkflow,
    jobs,
    job,
    image,
    ready: isReady(job, image),
    file,
    setFile,
    put,
    load,
  };
}
