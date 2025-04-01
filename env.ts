import { z } from "zod";

const envVariables = z.object({
  GEMINI_API_KEY: z.string(),
  AUTHORIZATION_KEY: z.string(),
  RATE_LIMIT_MAX: z.string(),
  RATE_LIMIT_DURATION: z.string(),
  PORT: z.string(),
  IS_BEHIND_CLOUDFLARE_TUNNEL: z.string(),
  POSTHOG_PROJECT_API_KEY: z.string().optional(),
  POSTHOG_API_HOST: z.string().optional(),
});

envVariables.parse(process.env);

declare global {
  namespace NodeJS {
    interface ProcessEnv extends z.infer<typeof envVariables> {}
  }
}
