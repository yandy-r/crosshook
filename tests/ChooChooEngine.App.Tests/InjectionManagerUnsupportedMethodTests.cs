using ChooChooEngine.App.Core;
using ChooChooEngine.App.Injection;

namespace ChooChooEngine.App.Tests;

public sealed class InjectionManagerUnsupportedMethodTests
{
	[Fact]
	public void Constructor_ThrowsImmediately_WhenProcessManagerIsNull()
	{
		ArgumentNullException exception = Assert.Throws<ArgumentNullException>(() => new InjectionManager(null));

		Assert.Equal("processManager", exception.ParamName);
	}

	[Theory]
	[InlineData(InjectionMethod.ManualMapping, "Manual mapping is not implemented")]
	[InlineData((InjectionMethod)999, "Injection method '999' is not supported.")]
	public void GetUnsupportedInjectionMethodMessage_ReturnsExpectedMessage(InjectionMethod method, string expected)
	{
		string message = InjectionManager.GetUnsupportedInjectionMethodMessage(method);

		Assert.Contains(expected, message);
	}
}
