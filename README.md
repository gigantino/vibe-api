# ✨ ~ v i b e - a p i ~ ✨

**Disclaimer: This project is an April Fools' joke.**

Do you consider yourself a [vibe coder](https://en.wikipedia.org/wiki/Vibe_coding)? Then why waste your precious vibe-checking energy building APIs when the vibes alone can give you everything you could ever dream of?

Forget documentation. Forget strict routing. With the power of ~vibes~, every endpoint is already there—waiting for you. Want your IP address? Hit up https://vibe-api.ggtn.ch/whats-my-ip, or honestly, misspell it wildly—it'll probably still vibe-check correctly. Need a random joke? Say less. https://vibe-api.ggtn.ch/random-joke will deliver one—most likely an unfunny one, but hey, it's the vibe that counts.

Is it fast? Absolutely not. But does it vibe? As long as you don't somehow confuse ChatGPT, the answer is a solid probably.

> [!WARNING]  
> Vibes are meant to be self-hosted. You're free to use my heavily rate-limited version, but just know that each request slowly drains my precious ChatGPT credits and my bank account is already begging for mercy.

## Installation & Usage

(Ensure you have [Bun](https://bun.sh/) installed on your system.)

1. **Clone the repository:**

   ```bash
   git clone https://github.com/gigantino/vibe-api.git
   ```

2. **Navigate to the project directory and install dependencies:**

```bash
cd vibe-api && bun install
```

3. **Copy and edit the environment variables:**

```bash
cp .env.example .env && vim .env # If you can't handle Vim, are you even worthy of the vibes?
```

4. **Install the packages and run the application:**

```bash
bun i && bun index.ts
```

## Special Headers

- **`X-VibeApi-Authorization`**
  Include your API key in this header to disable all rate limits for your requests.  
  _Example usage:_

  ```http
  X-VibeApi-Authorization: <your-api-key>
  ```

- **`X-VibeApi-Refresh`**  
  Set this header to `true` to force a refresh of the schema stored in the database. Useful when updates or changes have occurred.  
  _Example usage:_

  ```http
  X-VibeApi-Refresh: true
  ```

## Enjoy

⚠️ Always make sure to vibe with care ⚠️
