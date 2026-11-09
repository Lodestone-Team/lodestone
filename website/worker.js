// Cloudflare runs this only for requests that match no file in dist (static
// assets are served first, and _redirects before that).
//
// The dashboard at /host/app is a single-page app: react-router reads the
// path, so /host/app/login/core/new and the like have no file of their own and get the
// dashboard's page instead. Paths that look like files (a dot in the last
// segment) stay 404, so a missing script or image isn't answered with HTML.
export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const inDashboard = url.pathname === '/host/app' || url.pathname.startsWith('/host/app/');
    const lastSegment = url.pathname.slice(url.pathname.lastIndexOf('/') + 1);
    if (inDashboard && !lastSegment.includes('.')) {
      // "/host/app/" rather than "/host/app/index.html", which the asset
      // server would redirect to "/host/app/".
      const page = await env.ASSETS.fetch(new URL('/host/app/', url));
      return new Response(page.body, { status: 200, headers: page.headers });
    }
    return env.ASSETS.fetch(request);
  },
};
