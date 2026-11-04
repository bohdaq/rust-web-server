---
title: "Cookbook: PHP apps"
description: Laravel, Symfony and WordPress through php-fpm with rws's FastCGI support — no nginx or Apache.
---

rws talks to php-fpm directly ([FastCGI](/proxy/config-driven/#type--fastcgi--php-and-other-fastcgi-apps)).
Laravel or Symfony, whose web root is `public/` with a front controller:

```toml
port = 443
http_redirect_port = 80

[acme]
domains = ["shop.example.com"]
email   = "ops@example.com"

[[route]]
name = "laravel"
[route.match]
path = "/*"
[route.action]
type    = "fastcgi"
address = "unix:/run/php/php8.3-fpm.sock"
root    = "/var/www/shop/public"
[route.action.params]
APP_ENV = "production"
[route.middleware]
max_body_size = 33554432         # 32 MB uploads (match php.ini's upload_max_filesize)
```

CSS, JavaScript, images and uploaded files under `public/` are served by
rws itself; only PHP scripts reach php-fpm, and an upload can never be run
as PHP. php-fpm must be able to read the files: run its pool as the user
that owns them, and give rws read access too.

php-fpm is told each script's path under `root` as written in the config
(`SCRIPT_FILENAME` = root + script name, `DOCUMENT_ROOT` = root), with
symlinks left in, like nginx's `$document_root$fastcgi_script_name`. So
php-fpm in another container (a second container in the pod, or its own
Deployment) only needs the files mounted at the same path: a ConfigMap or
volume whose symlinks point somewhere else in each container works. rws
itself still resolves symlinks to check that a script is under the root.
Up to 18.27.2 the path was sent with symlinks resolved.

WordPress, with the admin area limited to the office and logins rate
limited:

```toml
port = 443
http_redirect_port = 80

[acme]
domains = ["blog.example.com"]
email   = "ops@example.com"

[[route]]
name = "wp-login"
[route.match]
path = "/wp-login.php"
[route.action]
type    = "fastcgi"
address = "127.0.0.1:9000"
root    = "/var/www/wordpress"
[route.middleware.rate_limit]
max_requests = 10
window_secs  = 60

[[route]]
name = "wp-admin"
[route.match]
path = "/wp-admin/*"
[route.action]
type    = "fastcgi"
address = "127.0.0.1:9000"
root    = "/var/www/wordpress"
[route.middleware.ip_filter]
allow = ["203.0.113.0/24"]

[[route]]
name = "wordpress"
[route.match]
path = "/*"
[route.action]
type    = "fastcgi"
address = "127.0.0.1:9000"
root    = "/var/www/wordpress"
```

Pretty permalinks work without rewrite rules: paths that match no file go
to `index.php` with the original `REQUEST_URI`.
