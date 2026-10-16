"""The console's FTP server (homebrew ftpsrv, port 2121), as the scripts here use it.

Paths are absolute: the server resolves from its root. Listing a path argument is unreliable,
so a listing changes directory first. A signed eboot.bin reads back larger than it was sent
(the server decodes the SELF), so arrival is checked by size only where sizes compare.
"""
import ftplib
import os


def connect() -> ftplib.FTP:
    host = os.environ.get("PS5_HOST")
    if not host:
        raise SystemExit("Set PS5_HOST to the console's address (see .env.example).")
    port = int(os.environ.get("PS5_FTP_PORT", "2121"))
    ftp = ftplib.FTP()
    try:
        ftp.connect(host, port, timeout=15)
    except OSError as e:
        raise SystemExit(f"Couldn't reach the console at {host}:{port}. Check it is on and its FTP server runs — {e}")
    ftp.login(os.environ.get("PS5_FTP_USER", "anonymous"), os.environ.get("PS5_FTP_PASSWORD", ""))
    ftp.voidcmd("TYPE I")
    return ftp


def title_dir(title_id: str) -> str:
    return f"/data/homebrew/{title_id}"


def mkdirs(ftp: ftplib.FTP, path: str) -> None:
    parts = [p for p in path.split("/") if p]
    for i in range(1, len(parts) + 1):
        try:
            ftp.mkd("/" + "/".join(parts[:i]))
        except ftplib.error_perm:
            pass  # 550: exists


def size(ftp: ftplib.FTP, path: str):
    try:
        return ftp.size(path)
    except ftplib.all_errors:
        return None


def upload(ftp: ftplib.FTP, local: str, remote: str) -> None:
    """Under a temporary name, then renamed: a half-written file is never the real one."""
    directory, name = remote.rsplit("/", 1)
    mkdirs(ftp, directory)
    tmp = f"{directory}/.{name}.upload"
    with open(local, "rb") as f:
        ftp.storbinary(f"STOR {tmp}", f)
    try:
        ftp.delete(remote)
    except ftplib.all_errors:
        pass
    ftp.rename(tmp, remote)


def listing(ftp: ftplib.FTP, path: str) -> list:
    try:
        ftp.cwd(path)
    except ftplib.error_perm:
        return []
    names = ftp.nlst()
    ftp.cwd("/")
    return [n for n in names if n not in (".", "..")]
