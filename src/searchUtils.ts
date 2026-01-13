/**
 * Search Utilities Module
 * 
 * Implements client-side re-ranking of search results using:
 * 1. Levenshtein Distance (fuzzy string matching)
 * 2. Token-based similarity scoring
 * 3. View count heuristic sorting
 */

import { VideoSearchResult } from "./SearchResultCard";

/**
 * Calculate the Levenshtein distance between two strings.
 * This is the minimum number of single-character edits (insertions, deletions, substitutions)
 * required to transform one string into the other.
 */
export function levenshteinDistance(str1: string, str2: string): number {
    const m = str1.length;
    const n = str2.length;

    // Create a 2D matrix for dynamic programming
    const dp: number[][] = Array.from({ length: m + 1 }, () =>
        Array(n + 1).fill(0)
    );

    // Initialize base cases
    for (let i = 0; i <= m; i++) dp[i][0] = i;
    for (let j = 0; j <= n; j++) dp[0][j] = j;

    // Fill the matrix
    for (let i = 1; i <= m; i++) {
        for (let j = 1; j <= n; j++) {
            if (str1[i - 1] === str2[j - 1]) {
                dp[i][j] = dp[i - 1][j - 1];
            } else {
                dp[i][j] = 1 + Math.min(
                    dp[i - 1][j],     // deletion
                    dp[i][j - 1],     // insertion
                    dp[i - 1][j - 1]  // substitution
                );
            }
        }
    }

    return dp[m][n];
}

/**
 * Calculate similarity ratio between two strings (0 to 1).
 * 1 = identical, 0 = completely different.
 */
export function similarityRatio(str1: string, str2: string): number {
    if (str1.length === 0 && str2.length === 0) return 1;
    if (str1.length === 0 || str2.length === 0) return 0;

    const distance = levenshteinDistance(str1.toLowerCase(), str2.toLowerCase());
    const maxLength = Math.max(str1.length, str2.length);

    return 1 - distance / maxLength;
}

/**
 * Tokenize a string into normalized words.
 * Removes punctuation, converts to lowercase, and filters short words.
 */
function tokenize(text: string): string[] {
    return text
        .toLowerCase()
        .replace(/[^\w\s]/g, " ")  // Remove punctuation
        .split(/\s+/)              // Split on whitespace
        .filter(token => token.length >= 2);  // Filter very short tokens
}

/**
 * Calculate token-based similarity between query and title.
 * 
 * Strategy: For each query token, find the best matching token in the title.
 * The overall similarity is the average of these best matches.
 * 
 * This handles partial matches well, e.g.:
 * - Query: "react tutorial"
 * - Title: "Complete React JS Tutorial for Beginners 2024"
 * - Tokens in title: ["complete", "react", "js", "tutorial", "for", "beginners", "2024"]
 * - "react" matches "react" (1.0), "tutorial" matches "tutorial" (1.0)
 * - Average: 1.0
 */
export function tokenSimilarity(query: string, title: string): number {
    const queryTokens = tokenize(query);
    const titleTokens = tokenize(title);

    if (queryTokens.length === 0) return 0;
    if (titleTokens.length === 0) return 0;

    let totalScore = 0;

    for (const qToken of queryTokens) {
        // Find the best matching title token for this query token
        let bestMatch = 0;

        for (const tToken of titleTokens) {
            // Check for substring match first (faster and handles common cases)
            if (tToken.includes(qToken) || qToken.includes(tToken)) {
                // Substring match - calculate based on overlap ratio
                const overlapLength = Math.min(qToken.length, tToken.length);
                const maxLength = Math.max(qToken.length, tToken.length);
                const substringScore = overlapLength / maxLength;
                bestMatch = Math.max(bestMatch, Math.max(substringScore, 0.8)); // Substring match is at least 0.8
            } else {
                // Fall back to Levenshtein for fuzzy matches
                const score = similarityRatio(qToken, tToken);
                bestMatch = Math.max(bestMatch, score);
            }
        }

        totalScore += bestMatch;
    }

    return totalScore / queryTokens.length;
}

/**
 * Re-rank search results using fuzzy matching and view count sorting.
 * 
 * @param results - Raw search results from backend
 * @param query - User's search query
 * @param threshold - Minimum similarity score (0-1) to include a result (default: 0.6)
 * @returns Filtered and sorted results
 */
export function reRankSearchResults(
    results: VideoSearchResult[],
    query: string,
    threshold: number = 0.6
): VideoSearchResult[] {
    // Calculate similarity for each result and filter
    const scoredResults = results
        .map(result => ({
            result,
            similarity: tokenSimilarity(query, result.title)
        }))
        .filter(item => item.similarity >= threshold);

    // Sort by view count (descending) - higher views first
    // For results with null view_count, treat as 0
    scoredResults.sort((a, b) => {
        const viewsA = a.result.view_count ?? 0;
        const viewsB = b.result.view_count ?? 0;
        return viewsB - viewsA;
    });

    // Extract just the results
    return scoredResults.map(item => item.result);
}

/**
 * Debug helper: Get detailed scoring info for search results.
 * Useful for tuning the algorithm.
 */
export function debugSearchScores(
    results: VideoSearchResult[],
    query: string
): Array<{ title: string; similarity: number; viewCount: number | null }> {
    return results.map(result => ({
        title: result.title,
        similarity: Math.round(tokenSimilarity(query, result.title) * 100) / 100,
        viewCount: result.view_count
    }));
}
