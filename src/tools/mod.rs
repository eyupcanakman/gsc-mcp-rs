mod analytics;
mod appearance;
mod discovery;
mod export;
mod indexing;
mod inspection;
mod meta;
mod properties;
mod sitemaps;

use crate::client::GscClient;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerInfo};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use std::sync::Arc;

// Re-import param types from sub-modules for use in Parameters<T>
use analytics::{
    ComparePeriodsParams, PageQueryBreakdownParams, PerformanceOverviewParams,
    SearchAnalyticsParams,
};
use appearance::SearchAppearanceParams;
use discovery::{
    BrandQueryParams, DetectAnomaliesParams, KeywordOpportunitiesParams, KeywordTrendParams,
    TopPagesParams,
};
use export::ExportAnalyticsParams;
use indexing::RequestIndexingParams;
use inspection::{BatchInspectParams, InspectUrlParams};
use properties::{ManageSiteParams, SiteUrlParams};
use sitemaps::{ListSitemapsParams, ManageSitemapParams};

pub struct GscServer {
    pub(crate) client: Arc<GscClient>,
    tool_router: rmcp::handler::server::router::tool::ToolRouter<Self>,
}

const SERVER_INSTRUCTIONS: &str = r"Google Search Console MCP Server (gsc-mcp-rs) -- 21 tools in 9 groups.

GETTING STARTED:
  - list_sites: Always start here to see available properties and your permission level.

SEARCH DATA:
  - search_analytics: Raw search data with all dimensions, filters, 25K rows.
    Use 'breakdown' param for device/country splits without changing dimensions.
  - keyword_opportunities: SEO opportunity finder with 5 modes (set mode param):
    quick_wins (default), cannibalization, ctr_gaps, declining, growing.
  - top_pages, brand_query_analysis, keyword_trend, detect_anomalies:
    Convenience tools for common SEO tasks.
  - compare_periods: Side-by-side comparison of two date ranges.
  - performance_overview: Quick aggregate summary of a site's performance.
  - page_query_breakdown: See which queries drive traffic to a specific page.
  - query_by_search_appearance: Filter data by rich result type (FAQ, VIDEO, etc).
  - export_analytics: Force CSV file export for large datasets.

INDEXING:
  - inspect_url / batch_inspect_urls: Check if Google has indexed specific URLs.
  - request_indexing: Ask Google to crawl a URL (separate API, requires indexing scope).

SITEMAPS:
  - list_sitemaps: List submitted sitemaps with status.
  - manage_sitemap(action: submit|delete): Submit or remove sitemaps.

PROPERTIES:
  - manage_site(action: add|delete): Add or remove GSC properties.
  - get_site_details: Get verification status and ownership details.
  - reauthenticate: Switch Google accounts.

META:
  - api_reference: Self-describing dimensions and metrics reference.

USAGE:
  - All date-accepting tools support 'days' param as shorthand (days: 28 = last 28 days).
  - Large results (>500 rows) are auto-exported to CSV with a summary returned inline.
  - Use filters with regex operators (includingRegex/excludingRegex) for pattern matching.";

impl GscServer {
    pub fn new(client: Arc<GscClient>) -> Self {
        let tool_router = Self::tool_router();
        Self {
            client,
            tool_router,
        }
    }
}

#[tool_router]
impl GscServer {
    // ── Properties ──────────────────────────────────────────────────────

    #[tool(
        description = "List all Google Search Console properties accessible with current credentials. Returns site URL, permission level (siteOwner/siteFullUser/siteRestrictedUser/siteUnverifiedUser). Always call this first to discover available properties."
    )]
    async fn list_sites(&self) -> String {
        self.handle_list_sites().await
    }

    #[tool(
        description = "Get verification status and ownership details for a specific property. Use this to check if a property is verified and what permission level you have."
    )]
    async fn get_site_details(&self, params: Parameters<SiteUrlParams>) -> String {
        self.handle_get_site_details(params.0).await
    }

    #[tool(
        description = "Add or remove a website property in Google Search Console. Action 'add' is idempotent (adding an existing site is safe). Action 'delete' removes from your dashboard only, not the actual website. Also idempotent."
    )]
    async fn manage_site(&self, p: Parameters<ManageSiteParams>) -> String {
        self.handle_manage_site(p.0).await
    }

    #[tool(
        description = "Clear saved OAuth tokens and trigger a fresh login. Use when switching Google accounts or if you see permission errors after account changes. In non-interactive mode, returns instructions to run 'gsc-mcp-rs auth' manually."
    )]
    async fn reauthenticate(&self) -> String {
        self.handle_reauthenticate().await
    }

    // ── Analytics ────────────────────────────────────────────────────────

    #[tool(
        description = "Raw search performance data. Supports all 7 dimensions (query, page, country, device, searchAppearance, date, hour), RE2 regex filters, multi-filter AND logic, up to 25,000 rows with pagination, sorting, data freshness control, and all search types including Discover. Use 'breakdown' param to add a comparison split (breakdown='device' splits by DESKTOP/MOBILE/TABLET). For common SEO tasks, prefer the convenience tools (top_pages, keyword_opportunities, etc.)."
    )]
    async fn search_analytics(&self, p: Parameters<SearchAnalyticsParams>) -> String {
        self.handle_search_analytics(p.0).await
    }

    #[tool(
        description = "Compare search performance between two date ranges side by side. Returns per-dimension metrics with absolute delta and percentage change. Use this for week-over-week, month-over-month, or year-over-year comparisons."
    )]
    async fn compare_periods(&self, p: Parameters<ComparePeriodsParams>) -> String {
        self.handle_compare_periods(p.0).await
    }

    #[tool(
        description = "Quick aggregate summary of a site's overall search performance: total clicks, impressions, average CTR, average position, plus daily trend data. Use this for a high-level health check before looking at specific dimensions."
    )]
    async fn performance_overview(&self, p: Parameters<PerformanceOverviewParams>) -> String {
        self.handle_performance_overview(p.0).await
    }

    #[tool(
        description = "See exactly which search queries drive traffic to a specific page. Use this to understand why a page ranks and which queries bring the most clicks. Different from search_analytics because it's focused on a single page URL."
    )]
    async fn page_query_breakdown(&self, p: Parameters<PageQueryBreakdownParams>) -> String {
        self.handle_page_query_breakdown(p.0).await
    }

    // ── Discovery ────────────────────────────────────────────────────────

    #[tool(
        description = "Get the highest-performing pages sorted by clicks, impressions, CTR, or position. Use this for a quick overview of your best content. For raw data with more control, use search_analytics with dimensions=['page']."
    )]
    async fn top_pages(&self, p: Parameters<TopPagesParams>) -> String {
        self.handle_top_pages(p.0).await
    }

    #[tool(
        description = "SEO opportunity finder with 5 analysis modes. Default 'quick_wins' finds keywords with high impressions but low CTR in positions 4-20. Other modes: 'cannibalization' (queries where 2+ pages compete), 'ctr_gaps' (underperforming CTR vs position benchmarks), 'declining' (biggest click losers vs previous period), 'growing' (biggest click gainers). Set the 'mode' param to switch."
    )]
    async fn keyword_opportunities(&self, p: Parameters<KeywordOpportunitiesParams>) -> String {
        self.handle_keyword_opportunities(p.0).await
    }

    #[tool(
        description = "Split search performance into brand vs non-brand traffic. You provide your brand terms (['acme', 'acmecorp']); the tool categorizes all queries and returns aggregate metrics for each group plus the brand percentage."
    )]
    async fn brand_query_analysis(&self, p: Parameters<BrandQueryParams>) -> String {
        self.handle_brand_query_analysis(p.0).await
    }

    #[tool(
        description = "Track daily performance of a single keyword over time. Returns date-by-date clicks, impressions, CTR, and position. Use this to monitor specific keyword movements after SEO changes."
    )]
    async fn keyword_trend(&self, p: Parameters<KeywordTrendParams>) -> String {
        self.handle_keyword_trend(p.0).await
    }

    #[tool(
        description = "Automatic anomaly detection: compares the last N days against the previous N days, flags significant drops in clicks/impressions or position regressions. Uses configurable severity thresholds (critical: 50%+ drop, warning: 20%+ drop)."
    )]
    async fn detect_anomalies(&self, p: Parameters<DetectAnomaliesParams>) -> String {
        self.handle_detect_anomalies(p.0).await
    }

    // ── Appearance ───────────────────────────────────────────────────────

    #[tool(
        description = "Filter search analytics by rich result type. Supported types: AMP_BLUE_LINK, AMP_TOP_STORIES, BREADCRUMB, EVENT, FAQ, HOWTO, IMAGE_PACK, JOB_LISTING, MERCHANT_LISTINGS, PRODUCT_SNIPPETS, RECIPE_FEATURE, RECIPE_RICH_SNIPPET, REVIEW_SNIPPET, SITELINKS, VIDEO, WEB_STORY. Use this to measure impact of structured data on search performance."
    )]
    async fn query_by_search_appearance(&self, p: Parameters<SearchAppearanceParams>) -> String {
        self.handle_query_by_search_appearance(p.0).await
    }

    // ── Export ────────────────────────────────────────────────────────────

    #[tool(
        description = "Force export search data to a CSV file regardless of row count. Bypasses the inline_limit threshold. Returns the file path and summary statistics. Use this when you need the full dataset for external analysis."
    )]
    async fn export_analytics(&self, p: Parameters<ExportAnalyticsParams>) -> String {
        self.handle_export_analytics(p.0).await
    }

    // ── Inspection ───────────────────────────────────────────────────────

    #[tool(
        description = "Check if Google has indexed a specific URL. Returns index status, last crawl time, crawl method, robots.txt state, mobile usability, and rich results detection. Configurable language for localized issue messages."
    )]
    async fn inspect_url(&self, p: Parameters<InspectUrlParams>) -> String {
        self.handle_inspect_url(p.0).await
    }

    #[tool(
        description = "Inspect up to 50 URLs in one call with configurable concurrency. Returns per-URL results plus an aggregated summary categorizing issues: indexed, not_indexed, canonical_issues, robots_blocked, fetch_errors. Logs progress to stderr during execution."
    )]
    async fn batch_inspect_urls(&self, p: Parameters<BatchInspectParams>) -> String {
        self.handle_batch_inspect_urls(p.0).await
    }

    // ── Sitemaps ─────────────────────────────────────────────────────────

    #[tool(
        description = "List all submitted sitemaps with status, error/warning counts, and last download time. Optionally filter by sitemap index URL."
    )]
    async fn list_sitemaps(&self, p: Parameters<ListSitemapsParams>) -> String {
        self.handle_list_sitemaps(p.0).await
    }

    #[tool(
        description = "Submit or remove a sitemap in Google Search Console. Action 'submit' is idempotent (resubmitting triggers a re-check). Action 'delete' is idempotent (deleting an already-removed sitemap returns success)."
    )]
    async fn manage_sitemap(&self, p: Parameters<ManageSitemapParams>) -> String {
        self.handle_manage_sitemap(p.0).await
    }

    // ── Indexing ─────────────────────────────────────────────────────────

    #[tool(
        description = "Request Google to crawl a URL via the Indexing API (separate from Search Console API). Supports URL_UPDATED (request indexing) and URL_DELETED (request removal). Note: requires the 'indexing' OAuth scope. Google deduplicates requests, so calling twice is safe."
    )]
    async fn request_indexing(&self, p: Parameters<RequestIndexingParams>) -> String {
        self.handle_request_indexing(p.0).await
    }

    // ── Meta ─────────────────────────────────────────────────────────────

    #[tool(
        description = "Self-describing API reference: lists all 7 dimensions (query, page, country, device, searchAppearance, date, hour) and all 4 metrics (clicks, impressions, ctr, position) with descriptions and value ranges."
    )]
    async fn api_reference(&self) -> String {
        self.handle_api_reference().await
    }
}

#[tool_handler]
impl ServerHandler for GscServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("gsc-mcp-rs", env!("CARGO_PKG_VERSION")))
            .with_instructions(SERVER_INSTRUCTIONS)
    }
}
