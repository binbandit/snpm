# Homepage Updates

## Changes Made

### 1. Dynamic Version from GitHub

The version displayed on the homepage is now fetched dynamically from the latest GitHub release.

**Implementation:**
- Created `/api/version` endpoint that fetches from GitHub API
- Caches the result for 18 hours (since releases happen once per day)
- Falls back to `2025.12.3` if the API call fails
- Version is displayed in the footer next to the snpm logo

**Files:**
- `src/app/api/version/route.ts` - API endpoint
- `src/app/(home)/page.tsx` - Updated Footer component to fetch and display version

### 2. Random Acronyms from Editable File

The random text (e.g., "Suddenly Not Panicking Manager") now comes from an editable text file.

**Implementation:**
- Created `public/snpm-acronyms.txt` with all acronyms (one per line)
- Created `/api/acronym` endpoint that reads the file and returns a random line
- Each page render fetches a new random acronym
- Falls back to "Suddenly Not Panicking Manager" if the file can't be read

**Files:**
- `public/snpm-acronyms.txt` - Editable list of acronyms
- `src/app/api/acronym/route.ts` - API endpoint
- `src/app/(home)/page.tsx` - Updated HeroSection to fetch acronym on render

## How to Edit Acronyms

Simply edit `docs-site/public/snpm-acronyms.txt` and add/remove/modify acronyms. Each line should contain one acronym. The changes will be reflected immediately on the next page load.

## Testing

Run the development server:
```bash
cd docs-site
pnpm dev
```

Visit http://localhost:3000 and refresh the page multiple times to see different acronyms.
