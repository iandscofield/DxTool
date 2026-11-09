/** Shop searches for a part. They open in the browser; DxTool never reads prices itself. */
export function shopLinks(search: string): { shop: string; url: string }[] {
  const q = encodeURIComponent(search).replace(/%20/g, "+");
  return [
    { shop: "PCPartPicker", url: `https://pcpartpicker.com/search/?q=${q}` },
    { shop: "Amazon", url: `https://www.amazon.com/s?k=${q}` },
    { shop: "Newegg", url: `https://www.newegg.com/p/pl?d=${q}` },
    { shop: "eBay", url: `https://www.ebay.com/sch/i.html?_nkw=${q}` },
  ];
}
