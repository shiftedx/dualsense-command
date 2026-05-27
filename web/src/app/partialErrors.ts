export function partialErrorArea(endpoint: string): string {
  if (endpoint.includes('controllers')) return 'controller diagnostics';
  if (endpoint.includes('steam-input')) return 'Steam Input layouts';
  if (endpoint.includes('input-bridge')) return 'DSCC Input Bridge';
  if (endpoint.includes('profiles')) return 'profiles';
  if (endpoint.includes('games') || endpoint.includes('steam-library')) return 'game library';
  if (endpoint.includes('support')) return 'support bundle';
  return 'agent data';
}

export function uniquePartialErrorAreas(endpoints: string[]): string[] {
  return endpoints
    .map((endpoint) => partialErrorArea(endpoint))
    .filter((area, index, areas) => areas.indexOf(area) === index);
}

