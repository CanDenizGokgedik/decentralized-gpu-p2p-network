/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'standalone',
  async rewrites() {
    return [
      {
        source: '/api/:path*',
        destination: 'http://localhost:8888/api/:path*',
      },
      {
        source: '/health',
        destination: 'http://localhost:8888/health',
      },
    ]
  },
  reactStrictMode: false,
}

module.exports = nextConfig
