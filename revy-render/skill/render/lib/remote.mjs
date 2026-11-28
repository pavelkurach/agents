// Pure: git remote URL -> forge, and forge + ref + path -> blob URL.

/** @returns {{kind:'github'|'gitlab'|'bitbucket', host:string, project:string} | undefined} */
export function parseRemote(url) {
  const m =
    /^(?:[a-z+]+:\/\/)?(?:[^@/]+@)?([^/:]+)(?::(?:\d+\/|\/)?|\/)(.+?)(?:\.git)?\/?$/i.exec(url.trim());
  if (!m) return undefined;
  const [, host, project] = m;
  const kind = forgeKind(host.toLowerCase());
  return kind && project.includes('/') ? { kind, host, project } : undefined;
}

function forgeKind(host) {
  if (host.includes('github')) return 'github';
  if (host === 'gitlab.com' || host.startsWith('gitlab.')) return 'gitlab';
  if (host.includes('bitbucket')) return 'bitbucket';
  return undefined;
}

const encodePath = (p) => p.split('/').map(encodeURIComponent).join('/');

/** @param {{start:number,end?:number}|undefined} lines */
export function blobUrl(remote, ref, filePath, lines) {
  const base = `https://${remote.host}/${remote.project}`;
  const file = `${encodeURIComponent(ref)}/${encodePath(filePath)}`;
  switch (remote.kind) {
    case 'github':
      return `${base}/blob/${file}${lines ? `#L${lines.start}${lines.end ? `-L${lines.end}` : ''}` : ''}`;
    case 'gitlab':
      return `${base}/-/blob/${file}${lines ? `#L${lines.start}${lines.end ? `-${lines.end}` : ''}` : ''}`;
    case 'bitbucket':
      return `${base}/src/${file}${lines ? `#lines-${lines.start}${lines.end ? `:${lines.end}` : ''}` : ''}`;
  }
}

export class LinkBaseError extends Error {}

/**
 * `--link <web-url>@<sha>`: the forge project and commit every code link of the primary repo points at.
 * @returns {{ remote: {kind:'github'|'gitlab'|'bitbucket', host:string, project:string}, ref: string }}
 */
export function parseLinkBase(arg) {
  const at = arg.lastIndexOf('@');
  const [url, ref] = at > 0 ? [arg.slice(0, at), arg.slice(at + 1)] : [arg, ''];
  if (!/^[\w./-]+$/.test(ref) || !/^https?:\/\//.test(url)) {
    throw new LinkBaseError(`--link ${arg}: expected <web-url>@<sha>, e.g. https://github.com/org/repo@1a2b3c4`);
  }
  const remote = parseRemote(url);
  if (!remote) throw new LinkBaseError(`--link ${arg}: not a GitHub, GitLab or Bitbucket project URL`);
  return { remote, ref };
}
