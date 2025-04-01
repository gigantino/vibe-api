import type { Generator } from "elysia-rate-limit";
import ipaddr from "ipaddr.js";

const cloudflareIPv4Ranges: string[] = [
  "173.245.48.0/20",
  "103.21.244.0/22",
  "103.22.200.0/22",
  "103.31.4.0/22",
  "141.101.64.0/18",
  "108.162.192.0/18",
  "190.93.240.0/20",
  "188.114.96.0/20",
  "197.234.240.0/22",
  "198.41.128.0/17",
  "162.158.0.0/15",
  "104.16.0.0/13",
  "104.24.0.0/14",
  "172.64.0.0/13",
  "131.0.72.0/22",
];

const cloudflareIPv6Ranges: string[] = [
  "2400:cb00::/32",
  "2606:4700::/32",
  "2803:f800::/32",
  "2405:b500::/32",
  "2405:8100::/32",
  "2a06:98c0::/29",
  "2c0f:f248::/32",
];

function isIpInRanges(ip: string, ranges: string[]): boolean {
  try {
    const parsedIp = ipaddr.parse(ip);
    return ranges.some((range) => {
      const [rangeAddress, prefixLengthStr] = range.split("/");
      const prefixLength = parseInt(prefixLengthStr, 10);
      const parsedRange = ipaddr.parse(rangeAddress);
      return (
        parsedIp.kind() === parsedRange.kind() &&
        parsedIp.match(parsedRange, prefixLength)
      );
    });
  } catch {
    return false;
  }
}

export const cloudflareGenerator: Generator = (req, server, _derived) => {
  const ip = server?.requestIP(req)?.address;
  if (!ip) return "";

  if (
    ipaddr.isValid(ip) &&
    (isIpInRanges(ip, cloudflareIPv4Ranges) ||
      isIpInRanges(ip, cloudflareIPv6Ranges))
  ) {
    return req.headers.get("CF-Connecting-IP") || ip;
  }

  return ip;
};
