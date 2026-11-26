// Copyright 2026 Seata-Rust contributors. Licensed under the Apache License, Version 2.0.
package sql

import (
	"context"
	"errors"
	"github.com/golang/mock/gomock"
	"seata.apache.org/seata-go/v2/pkg/rm"
	"strings"
	"sync"
	"testing"
)

const javaATLifecycleDSN = "user:owned-secret@tcp(127.0.0.1:1)/orders?timeout=20ms"
const javaATLifecycleURL = "jdbc:mysql://127.0.0.1:1/orders"

func TestJavaATDuplicateOpenCannotOverwriteExistingResource(t *testing.T) {
	ctrl := gomock.NewController(t)
	manager := newATCommitTestManager(t, ctrl)
	cache := &sync.Map{}
	old := &DBResource{resourceID: javaATLifecycleURL, javaATIdentity: true}
	cache.Store(javaATLifecycleURL, old)
	manager.EXPECT().GetCachedResources().Return(cache).AnyTimes()
	manager.EXPECT().RegisterResource(gomock.Any()).Times(0)
	got, err := OpenJavaATMySQL(javaATLifecycleDSN, javaATLifecycleURL)
	if err == nil || got != nil {
		t.Fatal("same-ID Open must reject before replacing registered resource")
	}
	actual, _ := cache.Load(javaATLifecycleURL)
	if actual != old {
		t.Fatal("failed second Open replaced old RM callback target")
	}
}
func TestJavaATFailedRegistrationRemovesOnlyItsOwnClosedPool(t *testing.T) {
	ctrl := gomock.NewController(t)
	manager := newATCommitTestManager(t, ctrl)
	cache := &sync.Map{}
	manager.EXPECT().GetCachedResources().Return(cache).AnyTimes()
	rejected := errors.New("owned registration rejection")
	var registered *DBResource
	manager.EXPECT().RegisterResource(gomock.Any()).DoAndReturn(func(resource rm.Resource) error {
		registered = resource.(*DBResource)
		cache.Store(resource.GetResourceId(), resource)
		return rejected
	})
	got, err := OpenJavaATMySQL(javaATLifecycleDSN, javaATLifecycleURL)
	if !errors.Is(err, rejected) || got != nil {
		t.Fatal("registration failure not returned")
	}
	if _, exists := cache.Load(javaATLifecycleURL); exists {
		t.Fatal("failed Open left callback cache pointing at closed direct pool")
	}
	if registered == nil {
		t.Fatal("actual registration path not reached")
	}
	if err = registered.db.Ping(); err == nil || !strings.Contains(err.Error(), "database is closed") {
		t.Fatal("failed Open did not close its owned pool")
	}
}
func TestJavaATFailedRegistrationCannotDeleteReplacementOwner(t *testing.T) {
	ctrl := gomock.NewController(t)
	manager := newATCommitTestManager(t, ctrl)
	cache := &sync.Map{}
	manager.EXPECT().GetCachedResources().Return(cache).AnyTimes()
	other := &DBResource{resourceID: javaATLifecycleURL}
	manager.EXPECT().RegisterResource(gomock.Any()).DoAndReturn(func(resource rm.Resource) error {
		cache.Store(resource.GetResourceId(), resource)
		cache.Store(resource.GetResourceId(), other)
		return errors.New("owned rejection after replacement")
	})
	_, err := OpenJavaATMySQL(javaATLifecycleDSN, javaATLifecycleURL)
	if err == nil {
		t.Fatal("rejection missing")
	}
	actual, _ := cache.Load(javaATLifecycleURL)
	if actual != other {
		t.Fatal("error cleanup deleted a replacement resource owner")
	}
}
func TestJavaATOuterCloseKeepsRuntimeOwnedCallbackPool(t *testing.T) {
	ctrl := gomock.NewController(t)
	manager := newATCommitTestManager(t, ctrl)
	cache := &sync.Map{}
	manager.EXPECT().GetCachedResources().Return(cache).AnyTimes()
	var registered *DBResource
	manager.EXPECT().RegisterResource(gomock.Any()).DoAndReturn(func(resource rm.Resource) error {
		registered = resource.(*DBResource)
		cache.Store(resource.GetResourceId(), resource)
		return nil
	})
	outer, err := OpenJavaATMySQL(javaATLifecycleDSN, javaATLifecycleURL)
	if err != nil {
		t.Fatal(err)
	}
	if err = outer.Close(); err != nil {
		t.Fatal(err)
	}
	actual, _ := cache.Load(javaATLifecycleURL)
	if actual != registered {
		t.Fatal("business handle close unregistered active callback resource")
	}
	err = registered.db.PingContext(context.Background())
	if err != nil && strings.Contains(err.Error(), "database is closed") {
		t.Fatal("business close invalidated late RM callbacks")
	}
	registered.db.Close()
	if registered.metaCache != nil {
		registered.metaCache.Destroy()
	}
}
