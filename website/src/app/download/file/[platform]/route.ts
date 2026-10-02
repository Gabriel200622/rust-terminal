import { downloadResponse } from "@/lib/download-route";
export async function GET(request: Request, context: { params: Promise<{ platform: string }> }) {
  return downloadResponse(request, (await context.params).platform, "stable");
}
