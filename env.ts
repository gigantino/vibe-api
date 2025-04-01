import { z } from "zod";

const envVariables = z.object({
  OPENAI_API_KEY: z.string(),
  AUTHORIZATION_KEY: z.string(),
  RATE_LIMIT_MAX: z.string(),
  RATE_LIMIT_DURATION: z.string(),
  PORT: z.string(),
});

envVariables.parse(process.env);

declare global {
  namespace NodeJS {
    interface ProcessEnv extends z.infer<typeof envVariables> {}
  }
}
