# 🐙 Automated CI/CD (GitHub Actions)

We have configured a fully automated pipeline using **GitHub Actions**. Whenever you push code to the `main` branch, it will automatically deploy your latest `web/` folder to Cloudflare Pages.

## How to activate this:

### 1. Push your code to GitHub
If you haven't already:
```bash
git add .
git commit -m "feat: setup github actions CI/CD"
git branch -M main
git remote add origin https://github.com/<YOUR_USERNAME>/<YOUR_REPO_NAME>.git
git push -u origin main
```

### 2. Add Secrets to GitHub
The GitHub action we just created (`.github/workflows/deploy.yml`) needs authorization to deploy to your Cloudflare account. 

Go to your GitHub Repository -> **Settings** -> **Secrets and variables** -> **Actions**. Add these two "New repository secrets":

1. **`CLOUDFLARE_ACCOUNT_ID`**
   - *How to get it:* Open your Cloudflare Dashboard, look at the URL `dash.cloudflare.com/{THIS_LONG_STRING}/pages`. Copy that exact string.
2. **`CLOUDFLARE_API_TOKEN`**
   - *How to get it:* In Cloudflare Dashboard, go to **My Profile** -> **API Tokens** -> **Create Token** -> Use the **Edit Cloudflare Workers** template -> Click **Continue to summary** and **Create Token**. Copy the secret.

### 3. Test the deployment
Once the secrets are pasted into GitHub, simply make a tiny edit to your HTML file locally, run `git add . && git commit -m "test deployment" && git push`, and watch the **Actions** tab on your GitHub repository automatically deploy it in less than 5 seconds!
