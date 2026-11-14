import { toast } from "sonner";
import { message } from "@/lib/message";

/** Copies `text` and says so, or says why it could not. */
export async function copy(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    toast.success("Copied");
  } catch (error) {
    toast.error("Could not copy", { description: message(error) });
  }
}
