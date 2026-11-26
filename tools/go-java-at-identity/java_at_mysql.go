// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package sql

import (
	"database/sql"
	"errors"
	"net"
	"net/url"
	"strconv"
	"strings"
	"sync"

	"github.com/go-sql-driver/mysql"
	"seata.apache.org/seata-go/v2/pkg/datasource/sql/datasource"
	"seata.apache.org/seata-go/v2/pkg/datasource/sql/types"
	"seata.apache.org/seata-go/v2/pkg/protocol/branch"
)

var javaATOpenMutex sync.Mutex

// OpenJavaATMySQL显式使用Java数据源的JDBC URL注册AT资源；既有DSN资源不会迁移。
// 调用方必须先停旧发起方并核验旧TC分支/Undo；本入口不推断主机别名或修改默认驱动。
// 返回DB的Close仅关闭业务句柄；已注册回调资源及其独立池由RM运行时持有至停机。
// 同一资源必须复用原句柄，禁止以第二次Open替换其回调目标。
func OpenJavaATMySQL(dsn, jdbcURL string) (*sql.DB, error) {
	resource, err := javaATResourceID(dsn, jdbcURL)
	if err != nil {
		return nil, err
	}
	javaATOpenMutex.Lock()
	defer javaATOpenMutex.Unlock()
	cache, err := javaATResourceCache()
	if err != nil {
		return nil, err
	}
	if _, exists := cache.Load(resource); exists {
		return nil, errors.New("Java AT: resource already registered; reuse its business handle and drain before shutdown")
	}
	cfg, err := mysql.ParseDSN(dsn)
	if err != nil {
		return nil, errors.New("Java AT: invalid MySQL connection configuration")
	}
	target, err := mysql.NewConnector(cfg)
	if err != nil {
		return nil, errors.New("Java AT: invalid MySQL connector configuration")
	}
	direct := sql.OpenDB(target)
	var ownedResource *DBResource
	adapter := &seataDriver{branchType: branch.BranchTypeAT, transType: types.ATMode, target: mysql.MySQLDriver{}, targetName: "mysql", descriptor: mySQLDriverDescriptor, javaATResourceID: resource, javaATResourceCreated: func(created *DBResource) { ownedResource = created }}
	connector, err := adapter.getOpenConnectorProxy(target, types.DBTypeMySQL, direct, dsn)
	if err != nil {
		// 缓存回滚必须核对本次创建的对象；不能删除其他所有者或留下关闭的池。
		if ownedResource != nil {
			cache.CompareAndDelete(resource, ownedResource)
			if ownedResource.metaCache != nil {
				ownedResource.metaCache.Destroy()
			}
		}
		direct.Close()
		return nil, err
	}
	return sql.OpenDB(&seataATConnector{seataConnector: connector.(*seataConnector), transType: types.ATMode}), nil
}

func javaATResourceCache() (cache *sync.Map, err error) {
	defer func() {
		if recover() != nil {
			cache = nil
			err = errors.New("Java AT: initialize the AT resource manager before opening a resource")
		}
	}()
	manager := datasource.GetDataSourceManager(branch.BranchTypeAT)
	if manager == nil || manager.GetCachedResources() == nil {
		return nil, errors.New("Java AT: initialized AT resource cache required")
	}
	return manager.GetCachedResources(), nil
}

// javaATResourceID只验证显式JDBC身份；错误不携带DSN、密码或URL参数。
func javaATResourceID(dsn, jdbcURL string) (string, error) {
	bad := errors.New("Java AT: JDBC identity does not match the MySQL endpoint and database")
	cfg, err := mysql.ParseDSN(dsn)
	if err != nil || cfg.Net != "tcp" || cfg.DBName == "" {
		return "", bad
	}
	if strings.ContainsAny(jdbcURL, "\r\n\x00") {
		return "", bad
	}
	identity := strings.SplitN(jdbcURL, "?", 2)[0]
	prefix := "jdbc:mysql://"
	loadbalance := strings.HasPrefix(identity, "jdbc:mysql:loadbalance://")
	if loadbalance {
		prefix = "jdbc:mysql:loadbalance://"
	}
	if !strings.HasPrefix(identity, prefix) {
		return "", bad
	}
	rest := strings.TrimPrefix(identity, prefix)
	slash := strings.Index(rest, "/")
	if slash < 1 {
		return "", bad
	}
	authorities, path := rest[:slash], rest[slash:]
	parsed, err := url.Parse("mysql://placeholder" + path)
	if err != nil || parsed.Fragment != "" || strings.TrimPrefix(parsed.Path, "/") != cfg.DBName {
		return "", bad
	}
	dsnHost, dsnPort, err := net.SplitHostPort(cfg.Addr)
	if err != nil {
		return "", bad
	}
	dsnNumber, err := strconv.Atoi(dsnPort)
	if err != nil || dsnNumber < 1 || dsnNumber > 65535 {
		return "", bad
	}
	matched := false
	for _, authority := range strings.Split(authorities, ",") {
		endpoint, parseErr := url.Parse("mysql://" + authority)
		if parseErr != nil || endpoint.User != nil || endpoint.Hostname() == "" || endpoint.Path != "" || endpoint.RawQuery != "" || endpoint.Fragment != "" {
			return "", bad
		}
		port := endpoint.Port()
		if port == "" {
			port = "3306"
		}
		number, parseErr := strconv.Atoi(port)
		if parseErr != nil || number < 1 || number > 65535 {
			return "", bad
		}
		if strings.EqualFold(endpoint.Hostname(), dsnHost) && number == dsnNumber {
			matched = true
		}
	}
	if !matched || (!loadbalance && strings.Contains(authorities, ",")) {
		return "", bad
	}
	if loadbalance {
		identity = strings.ReplaceAll(identity, ",", "|")
	}
	return identity, nil
}
