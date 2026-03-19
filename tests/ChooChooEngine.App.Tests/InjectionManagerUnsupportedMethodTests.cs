using ChooChooEngine.App.Injection;

namespace ChooChooEngine.App.Tests;

public sealed class InjectionManagerUnsupportedMethodTests
{
	[Theory]
	[InlineData(InjectionMethod.ManualMapping, "Manual mapping is not implemented")]
	[InlineData((InjectionMethod)999, "Injection method '999' is not supported.")]
	public void GetUnsupportedInjectionMethodMessage_ReturnsExpectedMessage(InjectionMethod method, string expected)
	{
		string message = InjectionManager.GetUnsupportedInjectionMethodMessage(method);

		Assert.Contains(expected, message);
	}
}
