import "./env";
import OpenAI from "openai";
import { Elysia, error } from "elysia";
import { rateLimit, defaultOptions } from "elysia-rate-limit";
import { staticPlugin } from "@elysiajs/static";
import { Database } from "bun:sqlite";
import { cloudflareGenerator } from "./cloudflare";

const openai = new OpenAI({
  apiKey: Bun.env.GEMINI_API_KEY,
  baseURL: "https://generativelanguage.googleapis.com/v1beta/openai",
});

const db = new Database("history.db");

db.run(`
    CREATE TABLE IF NOT EXISTS endpoint_schemas (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        endpoint_pattern TEXT UNIQUE,
        method TEXT,
        response_schema TEXT,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
`);

/** Strips away markdown from JSON responses */
const removeMarkdown = (input: string) => {
  const regex = /```(?:\w+\n)?([\s\S]*?)```/;
  const match = input.match(regex);
  return match ? match[1].trim() : input;
};

const app = new Elysia()
  .use(
    rateLimit({
      max: Number(Bun.env.RATE_LIMIT_MAX),
      duration: Number(Bun.env.RATE_LIMIT_DURATION) * 1000,
      errorResponse: new Response(
        `You are only allowed to send ${Bun.env.RATE_LIMIT_MAX} request(s) every ${Bun.env.RATE_LIMIT_DURATION} seconds`,
        {
          status: 429,
          headers: new Headers({
            "Content-Type": "text/plain",
          }),
        }
      ),
      generator: Bun.env.IS_BEHIND_CLOUDFLARE_TUNNEL
        ? cloudflareGenerator
        : defaultOptions.generator,
      skip: (request) => {
        const url = new URL(request.url);
        if (url.pathname === "/") return true;
        const authHeader = request.headers.get("X-VibeApi-Authorization");
        return authHeader === Bun.env.AUTHORIZATION_KEY;
      },
    })
  )
  .use(
    staticPlugin({
      prefix: "/",
      alwaysStatic: true,
    })
  )
  .get("/", async () => {
    return Bun.file("./public/index.html");
  })
  .all("*", async ({ request }: { request: Request }) => {
    const headersObj: Record<string, string> = {};
    request.headers.forEach((value, key) => {
      const lowerKey = key.toLowerCase();
      if (
        lowerKey === "x-vibeapi-authorization" ||
        lowerKey === "x-vibeapi-refresh"
      ) {
        return;
      }
      headersObj[key] = value;
    });
    const url = new URL(request.url);
    const endpointPattern = url.pathname;
    const fullPath = url.pathname + url.search;
    const method = request.method;
    const body = request.body ? await request.text() : null;

    const authHeader = request.headers.get("X-VibeApi-Authorization");
    const refreshHeader = request.headers.get("X-VibeApi-Refresh");
    const isAuthorized = authHeader === Bun.env.AUTHORIZATION_KEY;
    const shouldRefresh = isAuthorized && refreshHeader === "true";

    let existingSchema = null;
    if (!shouldRefresh) {
      existingSchema = db
        .query(
          `
            SELECT response_schema
            FROM endpoint_schemas
            WHERE endpoint_pattern = ?
                AND method = ?
          `
        )
        .get(endpointPattern, method) as { response_schema: string } | null;
    }

    let examplePrompt = "";

    if (existingSchema) {
      examplePrompt = `
        This endpoint has a specific schema that MUST be followed:
        ${existingSchema.response_schema}

        You MUST use exactly this schema structure, changing only the values to be appropriate for the current request parameters. The field names and nested structure must remain identical.
      `;
    }

    const systemPrompt = `
      You are a fake API response generator. Given an HTTP request, generate a realistic JSON response. The JSON response should just contain a body that makes sense for the specific response, and things like "status", "timestamp", "request_id" or other useless fields not directly related to the request shouldn't be returned.
      Do NOT include any markdown formatting or code blocks. Respond with raw JSON only. Strive not to return 'example.com' or something similar, you should strive for the links you are sending to be working.
    `;

    const userPrompt = `
      Generate a fake API response based on the following request:

      - Method: ${method}
      - Path: ${fullPath}
      - Headers: ${JSON.stringify(headersObj, null, 2)}
      - Body: ${body || "None"}

      ${examplePrompt}

      Reply ONLY with raw JSON. No explanation. No markdown.
    `;

    const completion = await openai.chat.completions.create({
      model: "gemini-2.0-flash",
      messages: [
        { role: "system", content: systemPrompt },
        { role: "user", content: userPrompt },
      ],
    });

    const responseText = completion.choices[0].message.content || "{}";

    try {
      const vibeResponse = JSON.parse(removeMarkdown(responseText));
      if (shouldRefresh) {
        db.run(
          `
            INSERT OR REPLACE INTO endpoint_schemas (endpoint_pattern, method, response_schema)
            VALUES (?, ?, ?)
          `,
          [endpointPattern, method, JSON.stringify(vibeResponse)]
        );
      } else if (!existingSchema) {
        db.run(
          `
            INSERT OR IGNORE INTO endpoint_schemas (endpoint_pattern, method, response_schema)
            VALUES (?, ?, ?)
          `,
          [endpointPattern, method, JSON.stringify(vibeResponse)]
        );
      }
      return vibeResponse;
    } catch (err) {
      return error(500, "Invalid AI response.");
    }
  })
  .listen(Bun.env.PORT);

console.info(`Running at http://${app.server?.hostname}:${app.server?.port}`);
